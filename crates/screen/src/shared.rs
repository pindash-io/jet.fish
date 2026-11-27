// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Screen-level shared state (theme, dimensions, title, clipboard, image
//! buffers). `ScreenShared<T>` is generic over the screen-specific extension.

use std::ops::{Deref, DerefMut};

use compact_str::CompactString;
use glam::USizeVec2;

use jetfish_command::Osc52;
use jetfish_protocol::{Grid, Theme};

/// Window-title state: the OSC 0/1/2 title, the CSI 22/23 push/pop stack, and
/// the XTITLEMODES encoding mode.
#[derive(Debug)]
pub struct WindowTitle {
    /// The current window/icon title (OSC 0/1/2), consumed once by the host.
    pub title: Option<CompactString>,
    /// Title stack for `CSI 22/23 t` (push/pop).
    pub stack: Vec<CompactString>,
    /// Encoding mode (`CSI > Ps t`): 0 = hex, 1 = UTF-8, 2 = unconditional UTF-8.
    pub mode: u8,
}

impl Default for WindowTitle {
    fn default() -> Self {
        Self { title: None, stack: Vec::new(), mode: 1 }
    }
}

/// OSC 52 clipboard state: the payload, the access policy, and any pending
/// read-query selector.
#[derive(Debug, Default)]
pub struct Clipboard {
    /// Clipboard payload set by OSC 52 (base64 bytes).
    pub payload: Option<Vec<u8>>,
    /// OSC 52 access policy.
    pub policy: Osc52,
    /// Queued OSC 52 read query selector.
    pub pending_query: Option<u8>,
}

/// Shared per-screen terminal state (not per-buffer).
#[derive(Debug)]
pub struct ScreenShared<T = ()> {
    /// The terminal theme (palette + defaults + highlight + detected scheme).
    pub theme: Theme,
    /// Terminal grid size in cells: `.x` = columns, `.y` = rows.
    pub grid: Grid,
    /// Terminal cell size in whole pixels, for CSI 14/16/18 replies and DEC
    /// 1016.
    pub cell: USizeVec2,
    /// Window title (text + CSI 22/23 stack + XTITLEMODES mode).
    pub window_title: WindowTitle,
    /// OSC 52 clipboard (payload + policy + pending read query).
    pub clipboard: Clipboard,
    /// Queued OSC 22 mouse cursor icon name.
    pub mouse_cursor: Option<CompactString>,
    /// Queued OSC 9 / OSC 777 desktop notification, consumed once.
    pub notification: Option<CompactString>,
    /// BEL (`0x07`) seen since the last consume, for the host to flash or beep.
    pub bell_pending: bool,
    /// Latest OSC 7501 program-status report, if any.
    pub program_status: Option<vtty_status::ProgramStatus>,
    /// Pending `CSI 8;rows;cols t` resize request for the host.
    pub pending_resize: Option<Grid>,
    /// Host reply queued for the PTY, flushed in `poll` so query bursts
    /// answer in order.
    pub pending_reply: Option<Vec<u8>>,
    /// PTY fd for query responses (`None` = headless/tests). The screen
    /// owns this duplicate so writes are safe.
    pub pty_fd: Option<std::os::fd::OwnedFd>,
    /// Terminal image infrastructure (decode buffers, cell size, parsers).
    pub image: ImageState,
    /// Screen-specific extension (cell metrics, reply queues, OSC overrides).
    pub extend: T,
    /// Set by ED-3 (`CSI 3J`); the engine drops the archived overflow.
    /// Consumed by `poll`.
    pub scrollback_erased: bool,
}

impl<T: Default> ScreenShared<T> {
    /// Fresh shared state for a `grid.x`×`grid.y` terminal.
    #[must_use]
    pub fn new(grid: Grid) -> Self {
        Self {
            theme: Theme::default(),
            grid: Grid::new(grid.x.max(1), grid.y),
            cell: USizeVec2::new(1, 1),
            window_title: WindowTitle::default(),
            clipboard: Clipboard::default(),
            mouse_cursor: None,
            notification: None,
            bell_pending: false,
            program_status: None,
            pending_resize: None,
            pending_reply: None,
            pty_fd: None,
            image: ImageState::default(),
            extend: T::default(),
            scrollback_erased: false,
        }
    }
}

/// Route screen-specific extension fields (`self.shared.cell.x`, …) straight
/// to the extension type.
impl<T> Deref for ScreenShared<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.extend
    }
}

impl<T> DerefMut for ScreenShared<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.extend
    }
}

/// Terminal image state (Sixel / kitty / iTerm2); the rendered per-buffer image
/// lives in the concrete [`State`](crate::State).
#[derive(Debug)]
pub struct ImageState {
    /// Buffer for a `DCS q` / `APC _G` image payload: `(final_byte, bytes)`.
    pub image_buf: Option<(u8, Vec<u8>)>,
    /// Buffer for a DCS query (`DCS $ q` DECRQSS / `DCS + q` XTGETTCAP):
    /// `(hex_encoded, payload)`. Buffered on hook, decoded on unhook.
    pub dcs_query: Option<(bool, Vec<u8>)>,
    /// The registered image parsers (kitty + Sixel built-ins, plus custom).
    pub parsers: jetfish_image::ImageParsers,
}

impl Default for ImageState {
    fn default() -> Self {
        Self {
            image_buf: None,
            dcs_query: None,
            parsers: jetfish_image::ImageParsers::default_parsers(),
        }
    }
}

impl ImageState {
    /// Begin a DCS/APC payload: a query (`DCS $ q` DECRQSS / `DCS + q`
    /// XTGETTCAP) or an image (`DCS q` / `APC _G`).
    pub fn begin(&mut self, ints: &[u8], final_byte: u8) {
        if let Some(hex) =
            jetfish_command::command::dcs_query_kind(ints, final_byte)
        {
            self.dcs_query = Some((hex, Vec::new()));
        } else {
            // Small sixel/kitty payloads dominate; reserve so the per-byte
            // push does not reallocate (~8 growths per short DCS otherwise).
            self.image_buf = Some((final_byte, Vec::with_capacity(256)));
        }
    }

    /// Push `byte` into the active DCS payload buffer (query or image),
    /// capped at [`jetfish_image::MAX_IMAGE_PAYLOAD`].
    pub fn push(&mut self, byte: u8) {
        self.push_slice(std::slice::from_ref(&byte));
    }

    /// Bulk append into the active DCS payload (query or image), capped at
    /// [`jetfish_image::MAX_IMAGE_PAYLOAD`]: one extend, not per-byte `put`.
    pub fn push_slice(&mut self, bytes: &[u8]) {
        let buf = self
            .dcs_query
            .as_mut()
            .map(|(_, b)| b)
            .or_else(|| self.image_buf.as_mut().map(|(_, b)| b));
        let Some(buf) = buf else { return };
        let rem = jetfish_image::MAX_IMAGE_PAYLOAD.saturating_sub(buf.len());
        buf.extend_from_slice(&bytes[..bytes.len().min(rem)]);
    }

    /// Take the buffered DCS query (`(hex_encoded, payload)`), if any.
    #[must_use]
    #[inline]
    pub const fn take_query(&mut self) -> Option<(bool, Vec<u8>)> {
        self.dcs_query.take()
    }

    /// Offer the buffered image payload to each registered parser, first
    /// recognizer winning: the shared `unhook` tail of both screens' dispatch.
    #[must_use]
    pub fn finish_image(
        &mut self,
        ctx: jetfish_image::ImageCtx,
    ) -> jetfish_image::ParseOutcome {
        let Some((final_byte, payload)) = self.image_buf.take() else {
            return jetfish_image::ParseOutcome::Skip;
        };
        let mut outcome = jetfish_image::ParseOutcome::Skip;
        for parser in self.parsers.iter_mut() {
            match parser.parse(&payload, final_byte, ctx) {
                jetfish_image::ParseOutcome::Skip => continue,
                done @ (jetfish_image::ParseOutcome::Consumed
                | jetfish_image::ParseOutcome::Handled(_)) => {
                    outcome = done;
                    break;
                }
            }
        }
        outcome
    }
}
