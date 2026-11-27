// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! SGR color resolution: palette ids to packed and float RGB, composed from
//! [`crate::attrs`]' effect primitives and [`super::Palette::resolve`].

use vtty::{ColorId, SgrFlags};
use vtty::{rgba_to_u32, u32_to_rgba};

use super::state::Palette;
use crate::Style;
use crate::attrs::{apply_sgr_effects, bold_to_bright, reverse_colors};

/// Resolve a cell or span's fg/bg RGB in canonical order (reverse, bold, bright,
/// resolve, then faint/invisible/inverse).
#[must_use]
#[inline]
pub fn resolve_colors(
    palette: &Palette,
    style: Style,
    reverse: bool,
    engine_fg: u32,
    engine_bg: u32,
    bold_is_bright: bool,
) -> (u32, u32) {
    let (fg, bg) = reverse_colors(style.fg, style.bg, reverse);
    let fg = bold_to_bright(
        fg,
        style.flags.contains(SgrFlags::BOLD),
        bold_is_bright,
    );
    let fg = palette.resolve(fg, engine_fg, engine_bg);
    let bg = palette.resolve(bg, engine_fg, engine_bg);
    apply_sgr_effects(
        fg,
        bg,
        style.flags.contains(SgrFlags::FAINT),
        style.flags.contains(SgrFlags::INVISIBLE),
        style.flags.contains(SgrFlags::INVERSE),
    )
}

/// Resolve a palette id to `[r, g, b, a]`, falling back to `engine_fg`/
/// `engine_bg` for the reserved default ids.
#[must_use]
#[inline]
pub fn resolve_rgba(
    palette: &Palette,
    id: ColorId,
    engine_fg: [f32; 4],
    engine_bg: [f32; 4],
) -> [f32; 4] {
    u32_to_rgba(palette.resolve(
        id,
        rgba_to_u32(engine_fg),
        rgba_to_u32(engine_bg),
    ))
}
