// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! [`BufferState`]: the per-buffer state (charset, margins, color stack,
//! shell activity, palette).

use vtty::{ColorId, CursorStyle, Scs, TermFlags};

use jetfish_command::ShellPrompt;
use jetfish_image::Image;
use jetfish_protocol::HashMap;
use jetfish_protocol::Margins;
use jetfish_protocol::Palette;
use jetfish_protocol::{Activity, Progress};

use crate::DEFAULT_FLAGS;

/// The per-buffer fields.
#[derive(Debug)]
pub struct BufferState {
    /// The DEC mode flags (`TermFlags`); main and alternate keep their own.
    pub flags: TermFlags,
    /// ISO 2022 SCS state (G0–G3 designations + GL/SS2/SS3 shifts).
    pub scs: Scs,
    /// DECSLRM horizontal margins (0-based, inclusive): `.x` = left,
    /// `.y` = right.
    pub margin: Margins,
    /// DECSCUSR cursor shape (`CSI Ps SP q`), per-buffer.
    pub cursor_shape: CursorStyle,
    /// xterm modifyOtherKeys mode (`CSI >4;m`): 0 off, 1/2 formats.
    pub modify_other_keys: u8,
    /// XTPUSHCOLORS/XTPOPCOLORS color stack (`CSI #P` / `CSI #Q`).
    pub color_stack: Vec<(ColorId, ColorId)>,
    /// Per-buffer themed palette override; `None` uses the shared default.
    pub palette: Option<Palette>,
    /// OSC 133 shell-integration state (FinalTerm/iTerm2 semantic prompts).
    pub shell: ShellPrompt,
    /// Shell running/idle state.
    pub activity: Activity,
    /// `OSC 9;4` progress-bar state.
    pub progress: Progress,
    /// The decoded terminal images (Sixel / kitty / iTerm2), keyed by the
    /// kitty image id (`0` for Sixel/iTerm2). Per-buffer.
    pub images: HashMap<u32, Image>,
    /// The most recently placed image id ([`BufferState::image`]).
    pub last_image_id: Option<u32>,
}

impl Default for BufferState {
    fn default() -> Self {
        Self {
            flags: DEFAULT_FLAGS,
            scs: Scs::default(),
            margin: Margins::default(),
            cursor_shape: CursorStyle::Block,
            modify_other_keys: 0,
            color_stack: Vec::new(),
            palette: None,
            shell: ShellPrompt::default(),
            activity: Activity::Unknown,
            progress: Progress::default(),
            images: HashMap::default(),
            last_image_id: None,
        }
    }
}

impl BufferState {
    /// A fresh `cols`-wide buffer with full-width margins. `Default` alone
    /// leaves the right margin at 0, so constructors go through here.
    #[inline]
    #[must_use]
    pub fn new(cols: usize) -> Self {
        Self {
            margin: Margins::new(0, cols.max(1).saturating_sub(1)),
            flags: DEFAULT_FLAGS,
            ..Self::default()
        }
    }

    /// The most recently placed image.
    #[inline]
    pub fn image(&self) -> Option<&Image> {
        self.last_image_id.and_then(|id| self.images.get(&id))
    }

    /// Every placed image, in arbitrary order (the render pass sorts by
    /// z-index before drawing).
    #[inline]
    pub fn images(&self) -> impl Iterator<Item = &Image> {
        self.images.values()
    }

    /// Place (or replace) an image by its id and record it as the latest.
    pub fn place_image(&mut self, image: Image) {
        let id = image.id;
        self.images.insert(id, image);
        self.last_image_id = Some(id);
    }

    /// Remove one image by id (kitty `a=d` with `d=i`/`d=I`).
    pub fn delete_image(&mut self, id: u32) {
        self.images.remove(&id);
        if self.last_image_id == Some(id) {
            self.last_image_id = None;
        }
    }

    /// Remove every image (kitty `a=d` with `d=a`, or ED).
    pub fn clear_images(&mut self) {
        self.images.clear();
        self.last_image_id = None;
    }

    /// Record an OSC 133 marker; returns the action byte.
    #[must_use]
    #[inline]
    pub fn apply_shell_prompt(&mut self, shell_prompt: ShellPrompt) -> u8 {
        self.shell = shell_prompt;
        // OSC 133 edges: `C` = running, `A`/`D` = idle; `B` (pre-exec) is not
        // an edge.
        match self.shell.action {
            b'C' => {
                self.activity = Activity::Running;
                // New command: clear the previous exit code.
                self.shell.exit_code = None;
            }
            b'A' | b'D' => self.activity = Activity::Idle,
            _ => {}
        }
        self.shell.action
    }

    /// Record an `OSC 9;4` progress state: the indeterminate and percent
    /// states mark the shell busy, the done/error/hidden states idle.
    pub const fn apply_progress(&mut self, progress: Progress) {
        self.progress = progress;
        self.activity = if matches!(
            progress,
            Progress::Indeterminate | Progress::Percent(_)
        ) {
            Activity::Running
        } else {
            Activity::Idle
        };
    }
}
