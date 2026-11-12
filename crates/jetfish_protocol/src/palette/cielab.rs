// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! CIELAB with a D65 reference white.

use ::color::{
    Chromaticity, ColorSpace, ColorSpaceLayout, OpaqueColor, Srgb, XyzD65,
};

/// CIE D65 white point (standard 2° observer).
const XN: f32 = 0.95047;
const ZN: f32 = 1.08883;

/// `(6/29)³` — the CIELAB linear/knee threshold.
const DELTA3: f32 = 216.0 / 24389.0;
const DELTA: f32 = 6.0 / 29.0;

/// Forward CIELAB companding.
///
/// Not `const`: [`f32::cbrt`] is not const-stable.
#[inline]
fn lab_f(t: f32) -> f32 {
    if t > DELTA3 { t.cbrt() } else { t / (3.0 * DELTA * DELTA) + 4.0 / 29.0 }
}

/// Inverse CIELAB companding.
#[inline]
const fn lab_f_inv(t: f32) -> f32 {
    if t > DELTA { t * t * t } else { 3.0 * DELTA * DELTA * (t - 4.0 / 29.0) }
}

/// CIELAB with a D65 reference white, the space the color256 algorithm
/// interpolates in.
#[derive(Clone, Copy, Debug)]
pub struct Cielab;

impl ColorSpace for Cielab {
    const LAYOUT: ColorSpaceLayout = ColorSpaceLayout::Rectangular;

    /// CIELAB is referenced against the D65 white point, like sRGB itself.
    /// Explicit rather than relying on the trait default so `_absolute`
    /// conversions and `chromatically_adapt` stay consistent.
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
    let r = ((rgb >> 16) & 0xff) as u8;
    let g = ((rgb >> 8) & 0xff) as u8;
    let b = (rgb & 0xff) as u8;
    OpaqueColor::<Srgb>::from_rgb8(r, g, b).convert::<Cielab>().components
}

/// CIELAB D65 `[L, a, b]` → sRGB `0xRRGGBB`.
#[inline]
pub(crate) fn lab_to_rgb(lab: [f32; 3]) -> u32 {
    let [r, g, b] =
        OpaqueColor::<Cielab>::new(lab).convert::<Srgb>().components;
    let pack = |c: f32| (c.clamp(0.0, 1.0) * 255.0).round() as u32;
    (pack(r) << 16) | (pack(g) << 8) | pack(b)
}

/// Linear interpolation in CIELAB D65.
#[inline]
pub(crate) fn lerp_lab(t: f32, a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    OpaqueColor::<Cielab>::new(a)
        .lerp_rect(OpaqueColor::<Cielab>::new(b), t)
        .components
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(got: u32, want: u32) {
        for shift in (0..24).step_by(8) {
            let d = (((got >> shift) & 0xff) as i32
                - ((want >> shift) & 0xff) as i32)
                .abs();
            assert!(d <= 2, "got {got:06x} want {want:06x}");
        }
    }

    #[test]
    fn lab_roundtrip_is_accurate() {
        for c in [
            0x000000, 0xFFFFFF, 0x100F0F, 0xFFFCF0, 0xAF3029, 0x205EA6,
            0x879A39, 0x3AA99F,
        ] {
            assert_close(lab_to_rgb(rgb_to_lab(c)), c);
        }
    }

    #[test]
    fn cielab_is_a_color_space() {
        // The crate machinery should round-trip through the trait.
        let lab =
            OpaqueColor::<Srgb>::new([1.0, 0.0, 0.0]).convert::<Cielab>();
        let back = lab.convert::<Srgb>();
        assert!(
            back.difference(OpaqueColor::<Srgb>::new([1.0, 0.0, 0.0])) < 1e-3
        );
    }
}
