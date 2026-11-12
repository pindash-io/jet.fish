// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! SGR color resolution: the canonical fg/bg pipeline from palette ids to
//! packed/float RGB, composed from [`crate::attrs`]' pure effect primitives
//! (`reverse_colors` / `bold_to_bright` / `apply_sgr_effects`) and
//! [`super::Palette::resolve`].

use crate::attrs::{apply_sgr_effects, bold_to_bright, reverse_colors};
use crate::{ColorId, SgrFlags, Style, rgba_to_u32, u32_to_rgba};

use super::state::Palette;

/// Resolve a cell/span's effective fg/bg RGB, applying the SGR modifiers in
/// the canonical order (reverse → bold→bright → resolve → faint/invisible/
/// inverse).
#[inline]
pub fn resolve_colors(
    palette: &Palette,
    style: Style,
    reverse: bool,
    engine_fg: u32,
    engine_bg: u32,
) -> (u32, u32) {
    let (fg, bg) = reverse_colors(style.fg, style.bg, reverse);
    let fg = bold_to_bright(fg, style.flags.contains(SgrFlags::BOLD));
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

/// Resolve a single palette id to `[r, g, b, a]`, using `engine_fg`/
/// `engine_bg` (float RGBA) as the fallbacks for the reserved default ids.
/// Composes [`Palette::resolve`] with the packed-RGB ↔ float-RGBA conversion
/// for the renderer's float-space callers (the SGR effects are applied
/// upstream by the screens, so this is a plain id→rgba lookup).
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
