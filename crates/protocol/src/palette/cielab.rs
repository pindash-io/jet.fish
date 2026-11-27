// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! CIELAB with a D65 reference white.

use color::{
    Chromaticity, ColorSpace, ColorSpaceLayout, OpaqueColor, Srgb, XyzD65,
};

use vtty::{rgba_to_u32, u32_to_array};

/// CIE D65 white point (standard 2° observer).
const XN: f32 = 0.95047;
const ZN: f32 = 1.08883;

/// `(6/29)³`: the CIELAB linear/knee threshold.
const DELTA3: f32 = 216.0 / 24389.0;
const DELTA: f32 = 6.0 / 29.0;

/// CIELAB (L\*a\*b\*) color space.
#[derive(Clone, Copy, Debug)]
pub struct Cielab;

impl ColorSpace for Cielab {
    const LAYOUT: ColorSpaceLayout = ColorSpaceLayout::Rectangular;

    /// CIELAB is referenced against D65, like sRGB: explicit rather than the
    /// trait default, so `_absolute` conversions stay consistent.
    const WHITE_POINT: Chromaticity = Chromaticity::D65;

    /// Lab white: `L = 100`, `a = b = 0`.
    const WHITE_COMPONENTS: [f32; 3] = [100.0, 0.0, 0.0];

    fn to_linear_srgb([l, a, b]: [f32; 3]) -> [f32; 3] {
        let fy = (l + 16.0) / 116.0;
        let fx = a / 500.0 + fy;
        let fz = fy - b / 200.0;
        let xyz = [lab_f_inv(fx) * XN, lab_f_inv(fy), lab_f_inv(fz) * ZN];
        XyzD65::to_linear_srgb(xyz)
    }

    fn from_linear_srgb(src: [f32; 3]) -> [f32; 3] {
        let xyz = XyzD65::from_linear_srgb(src);
        let fx = lab_f(xyz[0] / XN);
        let fy = lab_f(xyz[1]);
        let fz = lab_f(xyz[2] / ZN);
        [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
    }

    fn clip([l, a, b]: [f32; 3]) -> [f32; 3] {
        [l.clamp(0.0, 100.0), a, b]
    }

    fn scale_chroma([l, a, b]: [f32; 3], scale: f32) -> [f32; 3] {
        [l, a * scale, b * scale]
    }
}

/// sRGB `0xRRGGBB` → CIELAB D65 `[L, a, b]`.
#[inline]
pub(crate) fn rgb_to_lab(rgb: u32) -> [f32; 3] {
    let [r, g, b] = u32_to_array(rgb);
    OpaqueColor::<Srgb>::from_rgb8(r, g, b).convert::<Cielab>().components
}

/// CIELAB D65 `[L, a, b]` → sRGB `0xRRGGBBAA`.
#[inline]
pub(crate) fn lab_to_rgb(lab: [f32; 3]) -> u32 {
    let [r, g, b] =
        OpaqueColor::<Cielab>::new(lab).convert::<Srgb>().components;
    rgba_to_u32([r, g, b, 1.0])
}

/// Linear interpolation in CIELAB D65.
#[inline]
pub(crate) fn lerp_lab(t: f32, a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    OpaqueColor::<Cielab>::new(a)
        .lerp_rect(OpaqueColor::<Cielab>::new(b), t)
        .components
}

/// Forward companding: `t^(1/3)` above the knee, linear below it. Not
/// `const`, because [`f32::cbrt`] is not const-stable.
#[inline]
fn lab_f(t: f32) -> f32 {
    if t > DELTA3 { t.cbrt() } else { t / (3.0 * DELTA * DELTA) + 4.0 / 29.0 }
}

/// Inverse CIELAB companding.
#[inline]
const fn lab_f_inv(t: f32) -> f32 {
    if t > DELTA { t * t * t } else { 3.0 * DELTA * DELTA * (t - 4.0 / 29.0) }
}

#[cfg(test)]
#[path = "../../tests/palette/cielab.rs"]
mod tests;
