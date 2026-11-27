// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Generic dual-buffer screen shell: `Screen<S, T>` holds a main and
//! alternate buffer and derefs to the active one.

use std::ops::{Deref, DerefMut};
use std::os::fd::AsFd as _;
use std::sync::Arc;

use compact_str::CompactString;
use glam::USizeVec2;

use vtty::{CursorStyle, MouseTracking, Scs, TermFlags};

use jetfish_command::{ColorCommand, DefaultColor, Osc52, QueryContext};
use jetfish_protocol::{Activity, Grid, Progress};
use jetfish_protocol::{Defaults, Highlight, Palette};
use jetfish_protocol::{color_scheme, hex_decode, write_fd_all};

use crate::{BufferState, Mouse, ScreenShared};

/// A per-screen buffer type. The shared lifecycle lives here so `Screen` can
/// reset/resize both buffers uniformly.
pub trait State: Sized {
    /// A fresh buffer with the given terminal dimensions.
    fn fresh(grid: Grid) -> Self;

    /// Reset this single buffer to power-on (content + pen + modes).
    fn reset(&mut self, grid: Grid) {
        *self = Self::fresh(grid);
    }

    /// Resize the main buffer to at least `rows` (keep scrollback).
    fn resize_main(&mut self, rows: usize);

    /// Resize the alternate buffer to exactly `rows` (fixed grid).
    fn resize_alt(&mut self, rows: usize);

    /// Immutable access to the shared per-buffer shell state.
    fn buf(&self) -> &BufferState;

    /// Mutable access to the shared per-buffer shell state.
    fn buf_mut(&mut self) -> &mut BufferState;

    /// The per-buffer input-mode state (mouse, kitty keyboard, modifyOtherKeys),
    /// so the shell can answer mode queries without knowing the concrete state.
    fn mouse(&self) -> &Mouse;

    /// Mutable access to the per-buffer input-mode state.
    fn mouse_mut(&mut self) -> &mut Mouse;

    /// The buffer's shell activity state (OSC 133 A/C/D + OSC 9;4).
    fn activity(&self) -> Activity {
        self.buf().activity
    }

    /// The buffer's `OSC 9;4` progress-bar state.
    fn progress(&self) -> Progress {
        self.buf().progress
    }

    /// The last OSC 133 exit code (`D;<code>`), if any.
    fn shell_exit_code(&self) -> Option<i32> {
        self.buf().shell.exit_code
    }

    /// The last OSC 133 command line (`C;cmdline_url=…`), URL-decoded.
    fn shell_cmdline(&self) -> Option<&str> {
        self.buf().shell.cmdline.as_deref()
    }

    /// The DECRQSS ` q` cursor-style reply (`1`..`6` plus blink), the one shared
    /// arm of [`State::decrqss_value`].
    fn decrqss_cursor_style_value(&self) -> (bool, CompactString) {
        (
            true,
            jetfish_command::query::decrqss_cursor_style(
                self.buf().cursor_shape,
                self.buf().flags.contains(TermFlags::CURSOR_BLINK),
            ),
        )
    }

    /// Compute the DECRQSS reply value for a queried setting
    /// (`r`/`m`/` q`/`s`/`t`): returns `(valid, value)`.
    fn decrqss_value(
        &self,
        setting: &[u8],
        grid: Grid,
        palette: &[u32],
    ) -> (bool, CompactString);

    /// The per-buffer palette override, materialized from the shared theme on
    /// first use, so the shell can apply OSC colors without the concrete state.
    fn palette(&mut self) -> &mut Option<Palette> {
        &mut self.buf_mut().palette
    }

    /// The per-buffer palette override, split from [`State::palette`] so the
    /// shell can read it without a mutable borrow.
    fn palette_ref(&self) -> Option<&Palette> {
        self.buf().palette.as_ref()
    }

    /// The active kitty keyboard protocol flag bitfield (`0` = legacy).
    fn keyboard_flags(&self) -> u8;

    /// DECSTR / RIS: reset the kitty flags and the push/pop stack.
    fn keyboard_reset(&mut self) {}

    /// Whether DECCKM application cursor keys mode is active.
    fn application_cursor(&self) -> bool {
        self.buf().flags.contains(TermFlags::CURSOR_KEYS)
    }

    /// Whether DECKPAM application keypad mode is active.
    fn application_keypad(&self) -> bool {
        self.buf().flags.contains(TermFlags::KEYPAD_KEYS)
    }

    /// Whether DECBKM backarrow key mode is active (Backspace = `\x08`).
    fn backarrow(&self) -> bool {
        self.buf().flags.contains(TermFlags::BACKARROW_KEY_MODE)
    }

    /// Whether DEC 1035 numlock-ignore mode is active (the keypad is always
    /// numeric).
    fn numlock_ignored(&self) -> bool {
        self.buf().flags.contains(TermFlags::NUMLOCK)
    }

    /// Whether XTPUSHKEYS bracketed paste mode (`CSI ?2004h`) is active.
    fn bracketed_paste(&self) -> bool {
        self.buf().flags.contains(TermFlags::BRACKETED_PASTE)
    }

    /// Whether synchronized output mode (`CSI ?2026h`) is active.
    fn synchronized_output_active(&self) -> bool {
        self.buf().flags.contains(TermFlags::SYNC_OUTPUT)
    }

    /// The exclusive mouse tracking mode (0 = off, 9 = X10, 1000 = press,
    /// 1002 = drag, 1003 = any motion).
    fn mouse_mode(&self) -> u16;

    /// The mouse tracking modes (for SGR/X10 mouse reports).
    fn mouse_tracking(&self) -> MouseTracking;
}

/// Input accessors for a whole screen: the kitty/cursor/mouse ones delegate to
/// [`State`], and a concrete impl supplies the buffer accessor and cell size.
pub trait TermInput {
    /// The active buffer state (which implements [`State`]).
    type State: State;
    /// The active buffer state.
    fn state(&self) -> &Self::State;
    /// The grid cell size in whole pixels (for pixel mouse reports).
    fn cell_px(&self) -> USizeVec2;

    /// The active kitty keyboard protocol flag bitfield (`0` = legacy).
    fn keyboard_flags(&self) -> u8 {
        self.state().keyboard_flags()
    }

    /// Whether DECCKM application cursor keys mode is active.
    fn application_cursor(&self) -> bool {
        self.state().application_cursor()
    }

    /// Whether DECKPAM application keypad mode is active.
    fn application_keypad(&self) -> bool {
        self.state().application_keypad()
    }

    /// Whether DECBKM backarrow key mode is active (Backspace = `\x08`).
    fn backarrow(&self) -> bool {
        self.state().backarrow()
    }

    /// Whether DEC 1035 numlock-ignore mode is active.
    fn numlock_ignored(&self) -> bool {
        self.state().numlock_ignored()
    }

    /// Whether XTPUSHKEYS bracketed paste mode (`CSI ?2004h`) is active.
    fn bracketed_paste(&self) -> bool {
        self.state().bracketed_paste()
    }

    /// Whether synchronized output mode (`CSI ?2026h`) is active.
    fn synchronized_output_active(&self) -> bool {
        self.state().synchronized_output_active()
    }

    /// The exclusive mouse tracking mode (0 = off, 9 = X10, 1000 = press,
    /// 1002 = drag, 1003 = any motion).
    fn mouse_mode(&self) -> u16 {
        self.state().mouse_mode()
    }

    /// The mouse tracking modes (for SGR/X10 mouse reports).
    fn mouse_tracking(&self) -> MouseTracking {
        self.state().mouse_tracking()
    }
}

/// The main and alternate buffers plus the active flag; `Deref` routes field
/// access to the active buffer.
#[derive(Debug)]
pub struct Screen<S: State, T = ()> {
    /// The normal (main) buffer.
    pub main: S,
    /// The alternate buffer (`CSI ?47/1047/1049h`).
    pub alt: S,
    /// True while the alternate buffer is active.
    pub alt_active: bool,
    /// Shared per-screen state (palette, dimensions, title, clipboard, PTY)
    /// plus the screen-specific extension `T`.
    pub shared: ScreenShared<T>,
}

impl<S: State, T: Default> Screen<S, T> {
    /// A screen with two fresh buffers.
    #[must_use]
    pub fn new(grid: Grid) -> Self {
        Self {
            main: S::fresh(grid),
            alt: S::fresh(grid),
            alt_active: false,
            shared: ScreenShared::new(grid),
        }
    }
}

impl<S: State, T> Screen<S, T> {
    /// The active buffer.
    #[inline]
    pub const fn state(&self) -> &S {
        if self.alt_active { &self.alt } else { &self.main }
    }

    /// Mutable access to the active buffer.
    #[inline]
    pub const fn state_mut(&mut self) -> &mut S {
        if self.alt_active { &mut self.alt } else { &mut self.main }
    }

    /// True while the alternate buffer is active.
    #[inline]
    pub const fn is_alt(&self) -> bool {
        self.alt_active
    }

    /// Whether `mode` is set (DECRQM's backing query): mouse tracking, the
    /// alt-screen switch and every simple DEC mode resolve in one order here.
    pub fn mode_state(&self, mode: TermFlags) -> bool {
        if mode.is_empty() {
            return false; // TermFlags::NONE
        }
        if let Some(state) = self.state().mouse().mode_state(mode) {
            return state;
        }
        if mode.intersects(
            TermFlags::ALT_SCREEN
                | TermFlags::ALT_SCREEN_1047
                | TermFlags::ALT_SCREEN_1049,
        ) {
            return self.alt_active;
        }
        self.state().buf().flags.contains(mode)
    }

    /// DECRQM Pm value for a mode query (`CSI Ps$p` / `CSI ?Ps$p`):
    /// 0 = not recognized, 1 = set, 2 = reset.
    #[must_use]
    pub fn decrqm_state(&self, param: i32, private: bool) -> u8 {
        let mode = TermFlags::from_mode_code(param, private);
        jetfish_command::query::decrqm_status(mode, self.mode_state(mode))
    }

    /// The effective palette: the active buffer's themed override, else the
    /// shared theme default.
    #[inline]
    pub fn active_palette(&self) -> &Palette {
        self.state()
            .palette_ref()
            .unwrap_or_else(|| self.shared.theme.palette.as_ref())
    }

    /// Mutable access to the active buffer's themed palette, materializing a
    /// copy of the shared default on first mutation.
    #[inline]
    pub fn active_palette_mut(&mut self) -> &mut Palette {
        let Self { main, alt, alt_active, shared } = self;
        let state = if *alt_active { alt } else { main };
        materialize_palette(state, &shared.theme.palette);
        state.palette().as_mut().expect("palette just materialized")
    }

    /// Apply an OSC color command (4 / 10-19 / 104 / 110-119) to the active
    /// palette override and the shared theme, never the concrete text storage.
    pub fn apply_color(&mut self, c: ColorCommand) {
        let Self { main, alt, alt_active, shared } = self;
        let state = if *alt_active { alt } else { main };
        materialize_palette(state, &shared.theme.palette);
        apply_color_shared(
            state.palette().as_mut().expect("materialized above"),
            &mut shared.theme.defaults,
            &mut shared.theme.highlight,
            c,
        );
    }

    /// Apply an OSC 21 (kitty color protocol) payload: apply the `Set`/`Reset`
    /// ops and reply to the `Query` ops.
    pub fn apply_kitty_color(
        &mut self,
        ops: &[jetfish_command::command::KittyColorOp],
    ) {
        let reply = {
            let Self { main, alt, alt_active, shared } = self;
            let state = if *alt_active { alt } else { main };
            materialize_palette(state, &shared.theme.palette);
            jetfish_command::command::kitty_color_apply(
                state.palette().as_mut().expect("materialized above"),
                &mut shared.theme.defaults,
                &mut shared.theme.highlight,
                ops,
            )
        };
        let Some(reply) = reply else {
            return;
        };
        self.push_reply(reply.as_bytes());
    }

    /// `OSC 17`: selection highlight foreground color, if set.
    #[inline]
    pub const fn highlight_fg(&self) -> Option<u32> {
        self.shared.theme.highlight.fg
    }

    /// `OSC 19`: selection highlight background color, if set.
    #[inline]
    pub const fn highlight_bg(&self) -> Option<u32> {
        self.shared.theme.highlight.bg
    }

    /// Regenerate 256-color from the current base16 theme, when the host has
    /// palette generation enabled.
    pub fn regenerate_palette(&mut self, harmonious: bool) {
        let Some(palette) = Arc::get_mut(&mut self.shared.theme.palette)
        else {
            return;
        };
        palette.regenerate_from_theme(harmonious);
    }

    /// Set the XTITLEMODES encoding mode (`CSI > Ps t`).
    pub const fn set_title_mode(&mut self, mode: u8) {
        self.shared.window_title.mode = mode;
    }

    /// Set the window/icon title (OSC 0/1/2), hex-decoding it under XTITLEMODES
    /// mode 0; a plain title stores as-is, skipping the decode/re-encode.
    pub fn set_title(&mut self, title: CompactString) {
        self.shared.window_title.title = Some(
            if self.shared.window_title.mode == 0
                && title.as_bytes().len().is_multiple_of(2)
                && title.as_bytes().iter().all(u8::is_ascii_hexdigit)
            {
                CompactString::from_utf8_lossy(&hex_decode(title.as_bytes()))
            } else {
                title
            },
        );
    }

    /// Push the current title onto the `CSI 22 t` stack.
    pub fn push_title(&mut self) {
        let Some(t) = self.shared.window_title.title.clone() else {
            return;
        };
        self.shared.window_title.stack.push(t);
    }

    /// Pop and restore the title (`CSI 23 t`).
    pub fn pop_title(&mut self) {
        self.shared.window_title.title = self.shared.window_title.stack.pop();
    }

    /// Queue a desktop notification (OSC 9 / OSC 777 `notify`).
    pub fn notify(&mut self, message: CompactString) {
        self.shared.notification = Some(message);
    }

    /// Queue a titled desktop notification (OSC 99).
    pub fn notify_full(&mut self, title: String, body: String) {
        self.shared.notification =
            Some(match (title.is_empty(), body.is_empty()) {
                (false, false) => format!("{title}: {body}").into(),
                (false, true) => title.into(),
                _ => body.into(),
            });
    }

    /// Store an OSC 52 clipboard payload (base64 bytes).
    pub fn set_clipboard(&mut self, payload: &[u8]) {
        self.shared.clipboard.payload = Some(payload.to_vec());
    }

    /// Queue an OSC 22 mouse-cursor icon name.
    pub fn set_mouse_cursor(&mut self, name: CompactString) {
        self.shared.mouse_cursor = Some(name);
    }

    /// Queue a `CSI 8;rows;cols t` resize request for the host.
    pub const fn request_resize(&mut self, rows: usize, cols: usize) {
        self.shared.pending_resize = Some(Grid::new(cols, rows));
    }

    /// Switch to/from the alternate buffer (DEC 47/1047/1049): `clear` for
    /// `?1047/1049` (fresh each time), persistent for `?47`.
    pub fn switch_alt(&mut self, on: bool, clear: bool, grid: Grid) {
        if clear {
            self.alt = S::fresh(grid);
        }
        self.alt_active = on;
    }

    /// Resize both buffers to `rows` (main keeps scrollback, alt is fixed).
    pub fn resize(&mut self, rows: usize) {
        self.main.resize_main(rows);
        self.alt.resize_alt(rows);
    }

    /// Set the cell size in whole pixels (`.x` width, `.y` height), for
    /// `CSI 14/16/18 t` replies and DEC 1016 pixel mouse.
    pub fn set_cell_size(&mut self, w: usize, h: usize) {
        self.shared.cell = USizeVec2::new(w.max(1), h.max(1));
    }

    /// Compute the DECRQSS reply value for a queried setting.
    #[must_use]
    pub fn decrqss(
        &self,
        setting: &[u8],
        grid: Grid,
        palette: &[u32],
    ) -> (bool, CompactString) {
        self.state().decrqss_value(setting, grid, palette)
    }

    /// Build the DECRQSS reply (`DCS $ q … ST`) for a setting query.
    #[must_use]
    pub fn decrqss_reply(&self, setting: &[u8]) -> CompactString {
        let grid = self.shared.grid;
        let entries: Vec<u32> = self.active_palette().entries().to_vec();
        let (valid, value) = self.decrqss(setting, grid, &entries);
        vtty::Reply::decrqss(valid, &value)
    }

    /// Build the XTGETTCAP reply for a termcap query; only the reply
    /// transport differs.
    #[must_use]
    pub fn termcap_reply(&self, cap: &[u8]) -> CompactString {
        jetfish_command::query::termcap_reply(
            cap,
            self.shared.grid.x,
            self.shared.grid.y,
        )
    }

    /// Emit a DECRQM reply (`CSI Ps$p` / `CSI ?Ps$p`).
    pub fn reply_decrqm(&mut self, param: i32, private: bool) {
        let state = self.decrqm_state(param, private);
        let reply = vtty::Reply::decrqm(param, private, state);
        self.push_reply(reply.as_bytes());
    }

    /// Emit a DECRQSS reply through the shared reply transport.
    pub fn reply_decrqss(&mut self, setting: &[u8]) {
        let reply = self.decrqss_reply(setting);
        self.push_reply(reply.as_bytes());
    }

    /// Emit an XTGETTCAP reply through the shared reply transport.
    pub fn reply_termcap(&mut self, cap: &[u8]) {
        let reply = self.termcap_reply(cap);
        self.push_reply(reply.as_bytes());
    }

    /// Emit the XTPOPCOLORS color-stack depth report (`OSC 4` style, `CSI
    /// #Q` count) through the shared reply transport.
    pub fn reply_color_stack_report(&mut self) {
        let len = self.state().buf().color_stack.len();
        self.push_reply(
            jetfish_command::query::color_stack_report(len).as_bytes(),
        );
    }

    /// Reset both buffers to power-on and leave the alternate screen.
    pub fn reset_all(&mut self, grid: Grid) {
        self.alt_active = false;
        self.main.reset(grid);
        self.alt.reset(grid);
    }

    /// The shared DECSTR (`CSI ! p`) tail: DEC modes, mouse tracking, charset,
    /// DECSLRM margins, modifyOtherKeys and the image parsers. Each screen still
    /// resets its own storage (cursor, region, pen).
    pub fn soft_reset_shared(&mut self) {
        {
            let buf = self.state_mut().buf_mut();
            buf.flags.set(TermFlags::CURSOR_KEYS, false);
            buf.flags.set(TermFlags::KEYPAD_KEYS, false);
            buf.flags.set(TermFlags::BACKARROW_KEY_MODE, false);
            buf.flags.set(TermFlags::NUMLOCK, false);
            buf.flags.set(TermFlags::WRAPAROUND, true);
            buf.flags.set(TermFlags::INSERT, false);
            buf.flags.set(TermFlags::LINEFEED, false);
            buf.flags.set(TermFlags::CURSOR_VISIBLE, true);
            buf.flags.set(TermFlags::CURSOR_BLINK, true);
            buf.flags.set(TermFlags::ORIGIN, false);
            buf.flags.set(TermFlags::REVERSE_COLORS, false);
            buf.flags.set(TermFlags::REVERSE_WRAP, false);
            buf.flags.set(TermFlags::REVERSE_WRAP_EXTENDED, false);
            buf.flags.set(TermFlags::COLOR_SCHEME_UPDATES, false);
            buf.flags.set(TermFlags::BRACKETED_PASTE, false);
            buf.flags.remove(TermFlags::VI);
            buf.flags.set(TermFlags::ENABLE_LEFT_AND_RIGHT_MARGIN, false);
            buf.modify_other_keys = 0;
            buf.scs = Scs::ascii();
            buf.cursor_shape = CursorStyle::Block;
        }
        self.reset_left_right_margin();
        self.state_mut().mouse_mut().soft_reset();
        self.shared.image.parsers.clear();
    }

    /// Clear the displayed image (Sixel / kitty / iTerm2) so a later ED repaint
    /// starts clean. The kitty upload cache survives an erase; `a=d` deletes it.
    pub fn clear_image(&mut self) {
        self.state_mut().buf_mut().clear_images();
    }

    /// Reset the DECSLRM margins to the full viewport width.
    pub fn reset_left_right_margin(&mut self) {
        let right = self.shared.grid.x.max(1).saturating_sub(1);
        let buf = self.state_mut().buf_mut();
        buf.margin.x = 0;
        buf.margin.y = right;
    }

    /// Apply a DECSCUSR command (`CSI Ps SP q`): set the shape, ignoring
    /// unsupported codes, and track the DEC 12 blink flag on the buffer.
    pub fn apply_cursor_shape(&mut self, param: i32) {
        let shape = CursorStyle::from_decscusr(param);
        if shape != CursorStyle::None {
            self.state_mut().buf_mut().cursor_shape = shape;
        }
        let Some(blink) = CursorStyle::decscusr_blink(param) else {
            return;
        };
        self.state_mut().buf_mut().flags.set(TermFlags::CURSOR_BLINK, blink);
    }

    /// Reset the per-screen host-facing state: clipboard, notification,
    /// window title, mouse cursor, palette, and the scrollback-erase flag.
    pub fn reset_host_state(&mut self) {
        self.shared.clipboard.payload = None;
        self.shared.clipboard.pending_query = None;
        self.shared.notification = None;
        self.shared.window_title.title = None;
        self.shared.window_title.stack.clear();
        self.shared.mouse_cursor = None;
        *self.active_palette_mut() = Palette::default();
        self.shared.theme.defaults.cursor = self.shared.theme.defaults.fg;
        self.shared.scrollback_erased = true;
        self.shared.image.parsers.clear();
    }

    /// The active buffer's shell activity state (OSC 133 A/C/D + OSC 9;4).
    #[must_use]
    pub fn activity(&self) -> Activity {
        self.state().activity()
    }

    /// The active buffer's `OSC 9;4` progress-bar state.
    #[must_use]
    pub fn progress(&self) -> Progress {
        self.state().progress()
    }

    /// The active buffer's last OSC 133 exit code (`D;<code>`).
    #[must_use]
    pub fn shell_exit_code(&self) -> Option<i32> {
        self.state().shell_exit_code()
    }

    /// The active buffer's last OSC 133 command line (`C;cmdline_url=…`).
    pub fn shell_cmdline(&self) -> Option<&str> {
        self.state().shell_cmdline()
    }

    /// The OSC 52 clipboard payload (base64 bytes), consumed once.
    #[must_use]
    #[inline]
    pub const fn take_clipboard(&mut self) -> Option<Vec<u8>> {
        self.shared.clipboard.payload.take()
    }

    /// The queued window title (OSC 0/1/2), consumed once.
    #[must_use]
    #[inline]
    pub const fn take_title(&mut self) -> Option<CompactString> {
        self.shared.window_title.title.take()
    }

    /// The current window title (OSC 0/1/2), without consuming it.
    #[inline]
    pub fn title(&self) -> Option<&str> {
        self.shared.window_title.title.as_deref()
    }

    /// The queued `CSI 8;rows;cols t` resize request, consumed once.
    #[must_use]
    #[inline]
    pub const fn take_requested_resize(&mut self) -> Option<Grid> {
        self.shared.pending_resize.take()
    }

    /// Store an OSC 7501 program-status report.
    pub fn set_program_status(&mut self, status: vtty_status::ProgramStatus) {
        self.shared.program_status = Some(status);
    }

    /// The queued desktop notification (OSC 9 / OSC 777), consumed once.
    #[must_use]
    #[inline]
    pub const fn take_notification(&mut self) -> Option<CompactString> {
        self.shared.notification.take()
    }

    /// Whether a BEL (`0x07`) arrived since the last consume, and clear it.
    #[must_use]
    pub fn take_bell(&mut self) -> bool {
        std::mem::take(&mut self.shared.bell_pending)
    }

    /// The queued OSC 52 read query selector (`c`/`p`/`s`), consumed once.
    #[must_use]
    #[inline]
    pub const fn take_clipboard_query(&mut self) -> Option<u8> {
        self.shared.clipboard.pending_query.take()
    }

    /// Queue an OSC 52 read-query selector (`c`/`p`/`s`).
    pub const fn set_clipboard_query(&mut self, selector: u8) {
        self.shared.clipboard.pending_query = Some(selector);
    }

    /// Record the host-reported scheme (`1` dark, `2` light) and return the
    /// `CSI ?997;scheme n` report under DEC 2031.
    #[must_use]
    pub fn record_color_scheme(
        &mut self,
        scheme: u8,
        tracking: bool,
    ) -> Option<CompactString> {
        self.shared.theme.detected_scheme = Some(scheme);
        tracking.then(|| jetfish_command::query::color_scheme_report(scheme))
    }

    /// The effective color scheme (`1` dark, `2` light): the host-reported
    /// value when set, otherwise derived from the default background.
    #[must_use]
    pub fn color_scheme(&self) -> u8 {
        color_scheme(self.shared.theme.detected_scheme, self.default_bg_u32())
    }

    /// Record the host-reported scheme (`1` dark, `2` light) and return the
    /// `CSI ?997;scheme n` report for the host to write, under DEC 2031.
    #[must_use]
    pub fn set_color_scheme(&mut self, scheme: u8) -> Option<CompactString> {
        let tracking =
            self.state().buf().flags.contains(TermFlags::COLOR_SCHEME_UPDATES);
        self.record_color_scheme(scheme, tracking)
    }

    /// Whether DEC 2031 color-scheme tracking is enabled (`CSI ?2031h`).
    pub fn color_scheme_updates(&self) -> bool {
        self.state().buf().flags.contains(TermFlags::COLOR_SCHEME_UPDATES)
    }

    /// Whether DECOM origin mode is active (`CSI ?6h`).
    pub fn origin_mode(&self) -> bool {
        self.state().buf().flags.contains(TermFlags::ORIGIN)
    }

    /// Whether DECSCNM reverse video is active (`CSI ?5h`).
    pub fn reverse_video(&self) -> bool {
        self.state().buf().flags.contains(TermFlags::REVERSE_COLORS)
    }

    /// A DEC 1004 focus event (`CSI I` / `CSI O`) when focus reporting is on,
    /// for the host to write on a window focus change.
    #[must_use]
    pub fn report_focus(&self, focused: bool) -> Option<Vec<u8>> {
        self.state().mouse().focus_report(focused).map(<[u8]>::to_vec)
    }

    /// The resolved default foreground as packed RGB (the palette override,
    /// else the theme default).
    #[must_use]
    pub fn default_fg_u32(&self) -> u32 {
        self.active_palette()
            .default_fg()
            .unwrap_or(self.shared.theme.defaults.fg)
    }

    /// The resolved default background as packed RGB (the palette override,
    /// else the theme default).
    #[must_use]
    pub fn default_bg_u32(&self) -> u32 {
        self.active_palette()
            .default_bg()
            .unwrap_or(self.shared.theme.defaults.bg)
    }

    /// Build the [`QueryContext`](QueryContext) for a reply: `cursor` (1-based
    /// `(row, col)`) and `keyboard_flags` are the only per-storage inputs.
    #[must_use]
    pub fn query_context(
        &self,
        cursor: USizeVec2,
        keyboard_flags: u8,
    ) -> QueryContext {
        QueryContext {
            cursor,
            grid: self.shared.grid,
            cell: self.shared.cell,
            modify_other_keys: self.state().buf().modify_other_keys,
            keyboard_flags,
            color_scheme: self.color_scheme(),
            defaults: Defaults {
                fg: self.default_fg_u32(),
                bg: self.default_bg_u32(),
                cursor: self.shared.theme.defaults.cursor,
            },
        }
    }

    /// Set the OSC 52 access policy.
    pub const fn set_osc52(&mut self, policy: Osc52) {
        self.shared.clipboard.policy = policy;
    }

    /// The queued OSC 22 mouse cursor icon name, consumed once.
    pub const fn take_mouse_cursor(&mut self) -> Option<CompactString> {
        self.shared.mouse_cursor.take()
    }

    /// Send a host reply: straight to an attached PTY, else to `pending_reply`,
    /// which the in-memory harness asserts on. Order is kept either way.
    pub fn push_reply(&mut self, reply: &[u8]) {
        if self.shared.pty_fd.is_some() {
            self.write_pty(reply);
            return;
        }
        match &mut self.shared.pending_reply {
            Some(buf) => buf.extend_from_slice(reply),
            None => self.shared.pending_reply = Some(reply.to_vec()),
        }
    }

    /// Write raw bytes to the attached PTY (no-op when detached).
    pub fn write_pty(&self, data: &[u8]) {
        let Some(fd) = &self.shared.pty_fd else { return };
        write_fd_all(fd.as_fd(), data);
    }

    /// Take the queued host reply, if any (flushed to the PTY by `poll`).
    pub const fn take_pending_reply(&mut self) -> Option<Vec<u8>> {
        self.shared.pending_reply.take()
    }
}

impl<S: State, T> Deref for Screen<S, T> {
    type Target = S;

    fn deref(&self) -> &S {
        self.state()
    }
}

impl<S: State, T> DerefMut for Screen<S, T> {
    fn deref_mut(&mut self) -> &mut S {
        self.state_mut()
    }
}

impl<S: State, T> TermInput for Screen<S, T> {
    type State = S;

    fn state(&self) -> &S {
        Self::state(self)
    }

    fn cell_px(&self) -> USizeVec2 {
        self.shared.cell
    }
}

/// Materialize a buffer's palette override from the theme default on first
/// mutation.
fn materialize_palette<S: State>(state: &mut S, default: &Arc<Palette>) {
    let slot = state.palette();
    if slot.is_some() {
        return;
    }
    *slot = Some((**default).clone());
}

/// Apply an OSC color mutation to the shared palette and the default/highlight
/// colors, which both screens share, so it lives here once.
fn apply_color_shared(
    palette: &mut Palette,
    defaults: &mut Defaults<u32>,
    highlight: &mut Highlight<u32>,
    c: ColorCommand,
) {
    match c {
        ColorCommand::SetDefault { target, rgb } => match target {
            DefaultColor::Foreground => palette.set_default_fg(Some(rgb)),
            DefaultColor::Background => palette.set_default_bg(Some(rgb)),
            DefaultColor::Cursor => defaults.cursor = rgb,
        },
        ColorCommand::ResetDefault(target) => match target {
            DefaultColor::Foreground => palette.set_default_fg(None),
            DefaultColor::Background => palette.set_default_bg(None),
            DefaultColor::Cursor => defaults.cursor = defaults.fg,
        },
        ColorCommand::SetHighlight { target, rgb } => match target {
            DefaultColor::Foreground => highlight.fg = Some(rgb),
            DefaultColor::Background => highlight.bg = Some(rgb),
            DefaultColor::Cursor => {}
        },
        ColorCommand::ResetHighlight(target) => match target {
            DefaultColor::Foreground => highlight.fg = None,
            DefaultColor::Background => highlight.bg = None,
            DefaultColor::Cursor => {}
        },
        ColorCommand::SetPalette { index, rgb } => {
            palette.set_entry(index, rgb);
        }
        ColorCommand::SetPaletteBatch(entries) => {
            for (index, rgb) in entries {
                palette.set_entry(index, rgb);
            }
        }
        ColorCommand::ResetPalette(index) => {
            let Some(i) = index else {
                palette.reset();
                return;
            };
            palette.reset_entry(i);
        }
    }
}
