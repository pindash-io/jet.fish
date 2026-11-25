// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A deferred command queue. [`Commands`] buffers [`Command`]s and flushes them
//! via [`Commands::apply_to`]; it also implements [`Perform`], so a parser can
//! decode a stream into a list without a live screen.

use glam::USizeVec2;
use smallvec::SmallVec;

use vtty::Perform;
use vtty::{ColorId, SgrEffect};
use vtty::{DEFAULT_BG_ID, DEFAULT_FG_ID};

use jetfish_protocol::{CursorPos, Grid, Palette, Style};

use crate::command::{
    Command, Csi, CsiCtx, Esc, Move, Osc, Osc52, Sgr, resolve_spec,
};

/// A target that executes a semantic command. `CellScreen` implements
/// this; the trait exists so [`Commands`] can apply to a grid model.
pub trait Apply {
    /// Apply one decoded command.
    fn apply(&mut self, cmd: Command);

    /// Flush any buffered printable run before a control sequence is
    /// decoded. The cell screen batches text, so the default is a no-op.
    fn flush(&mut self) {}

    /// The CSI decode context (grid / scroll region / origin mode) used to
    /// normalize coordinates while parsing `CSI`.
    fn decode_ctx(&self) -> CsiCtx;

    /// The OSC 52 clipboard access policy used when decoding `OSC 52`.
    fn osc_policy(&self) -> Osc52;

    /// Direct SGR dispatch for the `CSI … m` hot path. Screens that decode
    /// SGR in place override this to skip the `Command`/`SmallVec` copy (a
    /// per-sequence alloc + drop); the default builds the owned [`Sgr`] and
    /// falls back to [`Apply::apply`].
    fn apply_sgr(&mut self, params: &[i32], sep: u16) {
        self.apply(Command::Sgr(Sgr::new(SmallVec::from(params), sep)));
    }

    /// Direct CUP dispatch for the `CSI r ; c H` hot path. `pos` is the
    /// 0-based `(col, row)`, already normalized (DECOM applied by the
    /// decoder). Screens override this to skip the `Command::Move`
    /// round-trip; the default falls back to [`Apply::apply`].
    fn move_to(&mut self, pos: CursorPos) {
        self.apply(Command::Move(Move::To(pos)));
    }

    /// Apply one parsed SGR effect to the screen's current style state via a
    /// [`Style`] round-trip instead of re-encoding a `Command` default per
    /// sequence.
    fn apply_effect(&mut self, e: SgrEffect) {
        if matches!(e, SgrEffect::Reset) {
            // Reset discards the current style entirely — skip the
            // `style()` extract + pack round-trip.
            self.set_style(Style::DEFAULT);
            return;
        }
        let mut style = self.style();
        match e {
            SgrEffect::Reset => unreachable!("handled above"),
            SgrEffect::Set(f) => style = style.apply_flags(f, true),
            SgrEffect::Clear(f) => style = style.apply_flags(f, false),
            SgrEffect::Underline(u) => style = style.with_underline_style(u),
            SgrEffect::Fg(spec) => {
                let c = resolve_spec(self.palette_mut(), spec, DEFAULT_FG_ID);
                style = style.with_fg(c);
            }
            SgrEffect::Bg(spec) => {
                let c = resolve_spec(self.palette_mut(), spec, DEFAULT_BG_ID);
                style = style.with_bg(c);
            }
            SgrEffect::UnderlineColor(spec) => {
                let def = style.fg;
                let c = resolve_spec(self.palette_mut(), spec, def);
                style = style.with_underline_color(Some(c));
            }
            SgrEffect::ClearUnderlineColor => {
                style = style.with_underline_color(None);
            }
        }
        self.set_style(style);
    }

    /// The current raw style (the cell screen projects its packed pen).
    fn style(&self) -> Style;

    /// Replace the current style (the cell screen repacks its pen, the line
    /// screen replaces its flat [`Style`]).
    fn set_style(&mut self, style: Style);

    /// Mutable access to the screen's palette for SGR color resolution.
    fn palette_mut(&mut self) -> &mut Palette;

    /// Mutable access to the XTPUSHCOLORS/XTPOPCOLORS color stack.
    fn color_stack_mut(&mut self) -> &mut Vec<(ColorId, ColorId)>;

    /// XTPUSHCOLORS (`CSI #P`): push the current fg/bg onto the color stack.
    fn push_colors(&mut self) {
        let style = self.style();
        self.color_stack_mut().push((style.fg, style.bg));
    }

    /// XTPOPCOLORS (`CSI #Q`): pop fg/bg off the color stack, if any.
    fn pop_colors(&mut self) {
        let Some((fg, bg)) = self.color_stack_mut().pop() else {
            return;
        };
        let style = self.style().with_fg(fg).with_bg(bg);
        self.set_style(style);
    }
}

// The `Perform` shims funnel into these one-liners so the parse → apply tail
// lives in exactly one place; screens still own their flush and
// pre-processing (CSI DEC aliases, alt-screen modes, DCS image payloads).

/// A buffer of [`Command`]s plus the decode context (`CsiCtx` / `Osc52`)
/// used when acting as a [`Perform`] sink. Push commands (or chain the fluent
/// helpers), then apply the batch via [`Self::apply_to`].
#[derive(Debug)]
pub struct Commands {
    queue: Vec<Command>,
    ctx: CsiCtx,
    osc52: Osc52,
}

impl Default for Commands {
    fn default() -> Self {
        Self {
            queue: Vec::new(),
            // A conventional 80×24 grid for screen-less decode.
            ctx: CsiCtx {
                size: Grid::new(80, 24),
                scroll: USizeVec2::new(0, 23),
                margin: USizeVec2::new(0, 79),
                decom: false,
            },
            osc52: Osc52::default(),
        }
    }
}

impl Commands {
    /// An empty command queue with the default decode context.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Buffer a command (chainable).
    pub fn push(&mut self, cmd: Command) -> &mut Self {
        self.queue.push(cmd);
        self
    }

    /// Buffer several commands (chainable).
    pub fn extend(
        &mut self,
        cmds: impl IntoIterator<Item = Command>,
    ) -> &mut Self {
        self.queue.extend(cmds);
        self
    }

    /// Whether no commands are buffered.
    pub const fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// The number of buffered commands.
    pub const fn len(&self) -> usize {
        self.queue.len()
    }

    /// Drop all buffered commands.
    pub fn clear(&mut self) {
        self.queue.clear();
    }

    /// Iterate the buffered commands (in push order).
    pub fn iter(&self) -> std::slice::Iter<'_, Command> {
        self.queue.iter()
    }

    /// Drain the buffered commands (in push order), leaving the queue empty.
    pub fn drain(&mut self) -> std::vec::Drain<'_, Command> {
        self.queue.drain(..)
    }

    /// Apply every buffered command to `target`, draining the queue.
    pub fn apply_to<T: Apply + ?Sized>(&mut self, target: &mut T) {
        for cmd in self.drain() {
            target.apply(cmd);
        }
    }

    /// CUP / HVP — move the cursor to `row`,`col` (0-based).
    pub fn move_to(&mut self, row: usize, col: usize) -> &mut Self {
        self.push(Command::Move(Move::To(CursorPos::new(col, row))))
    }

    /// ED — erase display (`0` below, `1` above, `2` all, `3` scrollback).
    pub fn erase_display(&mut self, mode: u8) -> &mut Self {
        self.push(Command::EraseDisplay(mode))
    }

    /// SU — scroll the region up `n` lines.
    pub fn scroll_up(&mut self, n: usize) -> &mut Self {
        self.push(Command::ScrollUp(n))
    }

    /// SD — scroll the region down `n` lines.
    pub fn scroll_down(&mut self, n: usize) -> &mut Self {
        self.push(Command::ScrollDown(n))
    }

    /// DECSTBM — set the scroll region (0-based, inclusive).
    pub fn set_scroll_region(
        &mut self,
        top: usize,
        bottom: usize,
    ) -> &mut Self {
        self.push(Command::SetScrollRegion { top, bottom })
    }

    /// DECSC — save the cursor.
    pub fn save_cursor(&mut self) -> &mut Self {
        self.push(Command::SaveCursor)
    }

    /// DECRC — restore the cursor.
    pub fn restore_cursor(&mut self) -> &mut Self {
        self.push(Command::RestoreCursor)
    }

    /// RIS — full reset.
    pub fn reset(&mut self) -> &mut Self {
        self.push(Command::Reset)
    }

    /// DECSTR — soft reset.
    pub fn soft_reset(&mut self) -> &mut Self {
        self.push(Command::SoftReset)
    }
}

/// `Commands` as a [`Perform`] sink: decodes CSI / ESC / OSC into the buffer
/// (using its `CsiCtx` / `Osc52`), dropping text and DCS passthrough.
impl Perform for Commands {
    fn print(&mut self, _c: char) {}
    fn print_bytes(&mut self, _s: &[u8]) {}
    fn execute(&mut self, _byte: u8) {}

    fn csi_dispatch(
        &mut self,
        params: &[i32],
        sep: u16,
        inters: &[u8],
        action: u8,
    ) {
        let Some(cmd) = Csi::parse(params, sep, inters, action, self.ctx)
        else {
            return;
        };
        self.queue.push(cmd);
    }

    fn esc_dispatch(&mut self, inters: &[u8], byte: u8) {
        let Some(cmd) = Esc::parse(inters, byte) else {
            return;
        };
        self.queue.push(cmd);
    }

    fn osc_dispatch(&mut self, params: &[&[u8]], _bell: bool) {
        let Some(cmd) = Osc::parse(params, self.osc52) else {
            return;
        };
        self.queue.push(cmd);
    }

    fn hook(
        &mut self,
        _params: &[i32],
        _intermediates: &[u8],
        _final_byte: u8,
    ) {
    }

    fn put(&mut self, _byte: u8) {}

    fn unhook(&mut self) {}
}

#[cfg(test)]
#[path = "../tests/commands/mod.rs"]
mod tests;
