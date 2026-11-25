// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! SGR (`CSI … m`) apply driver and palette resolution. The raw [`Sgr`] type
//! and its parsers live in `vtty`; this keeps the screen-specific apply driver.

use smallvec::SmallVec;

use vtty::{ColorId, ColorSpec, SgrEffect};
use vtty::{parse_effects, single_effect, two_effects};

use jetfish_protocol::Palette;

use crate::commands::Apply;

/// Apply every effect of an SGR sequence to `s`, in sequence order. The hot
/// single-attribute case uses [`vtty::single_effect`], the common
/// attribute+color pair uses [`vtty::two_effects`]; anything larger falls back
/// to [`vtty::parse_effects`] into a stack `SmallVec`.
#[inline]
pub fn for_each_effect<S: Apply + ?Sized>(
    s: &mut S,
    params: &[i32],
    sep: u16,
) {
    if let Some(e) = single_effect(params, sep) {
        s.apply_effect(e);
        return;
    }
    if let Some((e1, e2)) = two_effects(params, sep) {
        s.apply_effect(e1);
        s.apply_effect(e2);
        return;
    }
    let mut effects = SmallVec::<SgrEffect, 8>::new();
    parse_effects(params, sep, &mut effects);
    for e in effects {
        s.apply_effect(e);
    }
}

/// Resolve a parsed color spec against a palette. The `Ansi` arm leaves the
/// bold→bright legacy mapping to the caller (for the fg `30..=37` codes).
#[must_use]
#[inline]
pub fn resolve_spec(
    palette: &mut Palette,
    spec: ColorSpec,
    default: ColorId,
) -> ColorId {
    match spec {
        ColorSpec::Default => default,
        ColorSpec::Ansi(i) | ColorSpec::Indexed(i) => ColorId(i),
        ColorSpec::Rgb(rgb) => palette.insert_rgb(rgb),
    }
}

/// [`resolve_spec`] for the ANSI/256-color arms — these never touch the
/// palette, so the hot color-SGR path skips the `palette_mut()` materialize +
/// `Arc` clone. Callers route `ColorSpec::Rgb` to the palette themselves.
#[must_use]
#[inline]
pub fn resolve_color(spec: ColorSpec, default: ColorId) -> ColorId {
    match spec {
        ColorSpec::Default => default,
        ColorSpec::Ansi(i) | ColorSpec::Indexed(i) => ColorId(i),
        ColorSpec::Rgb(_) => unreachable!("Rgb resolves via the palette"),
    }
}
