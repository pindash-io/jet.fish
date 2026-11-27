// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Shared terminal-query reply byte strings and formatters.

use compact_str::CompactString;
use compact_str::format_compact;
use glam::USizeVec2;

use vtty::{ColorId, CursorStyle, SgrFlags, TermFlags, UnderlineStyle};
use vtty::{DEFAULT_BG_ID, DEFAULT_FG_ID};

use jetfish_protocol::{Defaults, Grid, Palette, Style};

use crate::command::{DefaultColor, Query};

/// The terminal name reported in the XTGETTCAP `TN` capability and the
/// XTVERSION reply.
pub const TERMINAL_NAME: &str = "jetfish";

/// The version reported in the XTVERSION reply.
pub const TERMINAL_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Screen-specific values a query reply needs, pre-computed by the screen
/// before calling [`query_reply`].
#[derive(Debug)]
pub struct QueryContext {
    /// 1-based `(row, col)` for DSR / extended cursor position.
    pub cursor: USizeVec2,
    /// Terminal grid size in cells (`.x` = cols, `.y` = rows).
    pub grid: Grid,
    /// Terminal cell size in whole pixels (`.x` = width, `.y` = height).
    pub cell: USizeVec2,
    /// `CSI >4m` modify-other-keys mode.
    pub modify_other_keys: u8,
    /// Active kitty keyboard flags (`CSI ?u`).
    pub keyboard_flags: u8,
    /// Detected color scheme (1 dark / 2 light).
    pub color_scheme: u8,
    /// Resolved default fg/bg/cursor (raw 0xRRGGBBAA) for OSC 10/11/12.
    pub defaults: Defaults<u32>,
}

impl QueryContext {
    /// Format a query reply; `None` for the payload-carrying queries the
    /// screens decode with their own state.
    #[must_use]
    pub fn query_reply(
        &self,
        q: &Query,
        palette: &Palette,
    ) -> Option<CompactString> {
        let s: CompactString = match q {
            Query::DeviceStatus(5) => CompactString::from("\x1b[0n"),
            Query::DeviceStatus(6) => {
                cursor_position_report(self.cursor.x, self.cursor.y)
            }
            Query::DeviceStatus(_) => return None,
            Query::ExtendedCursorPosition => {
                extended_cursor_position_report(self.cursor.x, self.cursor.y)
            }
            Query::DeviceAttrs(level) => match level {
                2 => vtty::Reply::da2(0, 136, 0),
                3 => vtty::Reply::da3(&[0, 0, 0, 0]),
                _ => vtty::Reply::da1(&[1, 2, 4]),
            },
            Query::Version => {
                vtty::Reply::xtversion(TERMINAL_NAME, TERMINAL_VERSION)
            }
            Query::ModifyOtherKeys => {
                modify_other_keys_report(self.modify_other_keys)
            }
            Query::KittyKeyboard => kitty_keyboard_report(self.keyboard_flags),
            Query::WindowSize { reply_code } => {
                let grid = self.grid.max(USizeVec2::ONE);
                let cell = self.cell.max(USizeVec2::ONE);
                let size = match reply_code {
                    8 => grid,
                    6 => cell,
                    _ => grid * cell,
                };
                window_size_report(*reply_code, size.y, size.x)
            }
            Query::WindowPosition => CompactString::from("\x1b[3;0;0t"),
            Query::WindowState => CompactString::from("\x1b[1t"),
            Query::DefaultColor(target) => {
                let (code, rgb) = match target {
                    DefaultColor::Foreground => (10, self.defaults.fg),
                    DefaultColor::Background => (11, self.defaults.bg),
                    DefaultColor::Cursor => (12, self.defaults.cursor),
                };
                vtty::format_osc_color_reply(code, rgb)
            }
            &Query::PaletteColor(i) => {
                let rgb = palette.get(ColorId::from(i)).unwrap_or(0);
                vtty::format_osc_palette_reply(i, rgb)
            }
            Query::ColorScheme => color_scheme_report(self.color_scheme),
            Query::Mode { .. }
            | Query::Setting(_)
            | Query::Termcap(_)
            | Query::Clipboard(_)
            | Query::ChecksumRect { .. } => return None,
        };
        Some(s)
    }
}

/// Format a cursor-position report (DSR 6, `CSI r;c R`) from 1-based coords.
#[must_use]
#[inline]
pub fn cursor_position_report(row: usize, col: usize) -> CompactString {
    format_compact!("\x1b[{row};{col}R")
}

/// Format an extended cursor-position report (DECXCPR, `CSI ? r;c R`).
#[must_use]
#[inline]
pub fn extended_cursor_position_report(
    row: usize,
    col: usize,
) -> CompactString {
    format_compact!("\x1b[?{row};{col}R")
}

/// Format the modifyOtherKeys report (`CSI ?4;mode m`).
#[must_use]
#[inline]
pub fn modify_other_keys_report(mode: u8) -> CompactString {
    format_compact!("\x1b[?4;{mode}m")
}

/// Format the kitty keyboard flags report (`CSI ?flags u`).
#[must_use]
#[inline]
pub fn kitty_keyboard_report(flags: u8) -> CompactString {
    format_compact!("\x1b[?{flags}u")
}

/// Format the XTCOLORMOD color-scheme report (`CSI ?997;scheme n`).
#[must_use]
#[inline]
pub fn color_scheme_report(scheme: u8) -> CompactString {
    format_compact!("\x1b[?997;{scheme}n")
}

/// Format the XTREPORTCOLORS reply (`CSI #R`) with the color-stack depth.
#[must_use]
#[inline]
pub fn color_stack_report(depth: usize) -> CompactString {
    format_compact!("\x1b[{depth}#R")
}

/// DECRQM status value: 0 = not recognized, 1 = set, 2 = reset.
#[must_use]
#[inline]
pub const fn decrqm_status(mode: TermFlags, set: bool) -> u8 {
    if mode.is_empty() {
        return 0;
    }
    if set { 1 } else { 2 }
}

/// DECRQSS `m` reply: the active SGR as a canonical sequence.
///
/// The underline style is emitted FIRST (no `0;` reset prefix) so nvim's
/// undercurl probe — it sets `4:3`, queries DECRQSS, and prefix-matches the
/// reply against `4:3m` — succeeds.
#[must_use]
#[inline]
pub fn decrqss_sgr(style: Style, palette: &[u32]) -> CompactString {
    let mut reply = CompactString::default();
    match style.underline_style {
        UnderlineStyle::None => {}
        UnderlineStyle::Single => reply.push_str("4"),
        UnderlineStyle::Double => reply.push_str("4:2"),
        UnderlineStyle::Curly => reply.push_str("4:3"),
        UnderlineStyle::Dotted => reply.push_str("4:4"),
        UnderlineStyle::Dashed => reply.push_str("4:5"),
        _ => {}
    }
    // The remaining flags (UNDERLINE is already covered by the style).
    for (flag, code) in [
        (SgrFlags::BOLD, ";1"),
        (SgrFlags::ITALIC, ";3"),
        (SgrFlags::INVERSE, ";7"),
    ] {
        if style.flags.contains(flag) {
            reply.push_str(code);
        }
    }
    // Colors, omitting the default fg/bg so the reply stays minimal.
    push_sgr_color(&mut reply, 3, style.fg, DEFAULT_FG_ID, palette);
    push_sgr_color(&mut reply, 4, style.bg, DEFAULT_BG_ID, palette);
    if reply.is_empty() {
        reply.push('0');
    }
    reply.push('m');
    reply
}

/// Append one SGR color code, skipping the default color entirely (a minimal
/// reply, not the always-`39`/`49` form).
fn push_sgr_color(
    s: &mut CompactString,
    base: u8,
    id: ColorId,
    default_id: ColorId,
    palette: &[u32],
) {
    use std::fmt::Write as _;

    if id == default_id {
        return;
    }
    if id.0 <= 255 {
        let _ = write!(s, ";{base}8;5;{}", id.0);
        return;
    }
    if let Some(rgb) = palette.get(id.0 as usize).copied() {
        let [r, g, b] = vtty::u32_to_array(rgb);
        let _ = write!(s, ";{base}8;2;{r};{g};{b}");
    }
}

/// DECRQSS ` q` reply: the cursor shape (and blink).
#[must_use]
#[inline]
pub fn decrqss_cursor_style(style: CursorStyle, blink: bool) -> CompactString {
    format_compact!("{} q", style.to_decscusr(blink))
}

/// DECRQSS `t` reply: tab stops, 1-based columns joined by `;`.
#[must_use]
#[inline]
pub fn decrqss_tab_stops(stops: impl Iterator<Item = usize>) -> CompactString {
    use std::fmt::Write as _;

    let mut out = CompactString::default();
    for (i, c) in stops.enumerate() {
        if i > 0 {
            out.push(';');
        }
        let _ = write!(out, "{}", c + 1);
    }
    out.push('t');
    out
}

/// Format a window-size report (`CSI {code};{rows};{cols} t`).
#[must_use]
#[inline]
pub fn window_size_report(
    code: u8,
    rows: usize,
    cols: usize,
) -> CompactString {
    format_compact!("\x1b[{code};{rows};{cols}t")
}

/// Format a DEC 2048 in-band resize report (`CSI 48;lines;cols;pixel_lines;
/// pixel_cols t`).
#[must_use]
#[inline]
pub fn in_band_resize_report(
    rows: u32,
    cols: u32,
    pixel_h: u32,
    pixel_w: u32,
) -> CompactString {
    format_compact!("\x1b[48;{rows};{cols};{pixel_h};{pixel_w}t")
}

/// Format a DECRQCRA checksum report (`DCS {id}!~{checksum} ST`).
#[must_use]
#[inline]
pub fn decrcra_reply(id: i32, checksum: u32) -> CompactString {
    format_compact!("\x1bP{id}!~{checksum:04X}\x1b\\")
}

/// Compute and format a DECRQCRA report.
#[must_use]
#[inline]
pub fn decrcra_reply_for(
    grid: Grid,
    id: i32,
    top: usize,
    left: usize,
    bottom: usize,
    right: usize,
    mut row_text: impl FnMut(usize) -> Option<String>,
) -> CompactString {
    let (top, left, bottom, right) = jetfish_protocol::clamp_rect_to_page(
        top, left, bottom, right, grid.x, grid.y,
    );
    let checksum = jetfish_protocol::decrcra_checksum(
        &mut row_text,
        top.saturating_sub(1),
        left.saturating_sub(1),
        bottom.saturating_sub(1),
        right.saturating_sub(1),
    );
    decrcra_reply(id, checksum)
}

/// Format an XTGETTCAP reply for a queried capability (shared terminfo table;
/// multi-cap `Tc;RGB;setrgbf;setrgbb` supported).
#[must_use]
#[inline]
pub fn termcap_reply(cap: &[u8], cols: usize, rows: usize) -> CompactString {
    vtty::Reply::termcap(
        cap,
        cols.max(1),
        rows.max(1),
        TERMINAL_NAME.as_bytes(),
        std::env::consts::OS.as_bytes(),
    )
}
