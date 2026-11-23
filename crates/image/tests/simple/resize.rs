// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::scale_down_rgba;

/// A 2×2 image downscaled to fit 1×1 halves each axis.
#[test]
fn scales_down_and_averages_rgba() {
    // 2×2 opaque: red, green / blue, white.
    let px = [
        255, 0, 0, 255, //
        0, 255, 0, 255, //
        0, 0, 255, 255, //
        255, 255, 255, 255,
    ];
    let (out, w, h) = scale_down_rgba(&px, 2, 2, 1, 1).expect("must scale");
    assert_eq!((w, h), (1, 1));
    // Average of the four corners = (127, 127, 127) (integer 510/4).
    assert_eq!(&out[..3], &[127, 127, 127]);
    assert_eq!(out[3], 255);
}

/// Already-fitting images return `None` (no resize).
#[test]
fn fits_images_are_untouched() {
    let px = [1, 2, 3, 4];
    assert!(scale_down_rgba(&px, 1, 1, 512, 512).is_none());
}

/// A wide image scales by width; the aspect ratio is preserved.
#[test]
fn wide_image_scales_to_max_width() {
    // 4×2 opaque green.
    let px = [0, 255, 0, 255].repeat(4 * 2);
    let (out, w, h) = scale_down_rgba(&px, 4, 2, 2, 512).expect("must scale");
    assert_eq!((w, h), (2, 1));
    // Both output pixels average the whole green row → opaque green.
    assert_eq!(&out[..4], &[0, 255, 0, 255]);
    assert_eq!(&out[4..], &[0, 255, 0, 255]);
}

/// Transparent pixels must not smear colour into an opaque neighbour.
#[test]
fn transparent_pixels_do_not_smear() {
    // 2×1: fully-transparent red next to opaque white.
    let px = [255, 0, 0, 0, 255, 255, 255, 255];
    let (out, w, h) = scale_down_rgba(&px, 2, 1, 1, 1).expect("must scale");
    assert_eq!((w, h), (1, 1));
    // Alpha-weighted colour = white (the transparent red contributes 0).
    assert_eq!(&out[..3], &[255, 255, 255]);
    assert_eq!(out[3], 127); // (0 + 255) / 2
}

/// The SIMD path (a multi-row, non-4-multiple box) must match a naive
/// scalar reference on an odd-sized downscale.
#[test]
fn simd_matches_scalar_reference() {
    let (w, h) = (7_u32, 5_u32);
    let mut px = vec![0_u8; (w * h * 4) as usize];
    for i in 0..(w * h) as usize {
        px[i * 4] = (i as u8).wrapping_mul(37);
        px[i * 4 + 1] = (i as u8).wrapping_mul(91);
        px[i * 4 + 2] = (i as u8).wrapping_mul(13);
        px[i * 4 + 3] = 255;
    }
    let (out, nw, nh) = scale_down_rgba(&px, w, h, 3, 2).expect("must scale");
    assert_eq!((nw, nh), (2, 2));
    // Naive box-filter reference (straight alpha, all opaque).
    for dy in 0..nh {
        let sy0 = dy * h / nh;
        let sy1 = (dy + 1) * h / nh;
        for dx in 0..nw {
            let sx0 = dx * w / nw;
            let sx1 = (dx + 1) * w / nw;
            let mut acc = [0_u64; 3];
            let mut count = 0_u64;
            for sy in sy0..sy1 {
                for sx in sx0..sx1 {
                    let idx = ((sy * w + sx) * 4) as usize;
                    acc[0] += u64::from(px[idx]);
                    acc[1] += u64::from(px[idx + 1]);
                    acc[2] += u64::from(px[idx + 2]);
                    count += 1;
                }
            }
            let oidx = ((dy * nw + dx) * 4) as usize;
            assert_eq!(out[oidx], (acc[0] / count) as u8, "r at ({dx},{dy})");
            assert_eq!(
                out[oidx + 1],
                (acc[1] / count) as u8,
                "g at ({dx},{dy})"
            );
            assert_eq!(
                out[oidx + 2],
                (acc[2] / count) as u8,
                "b at ({dx},{dy})"
            );
            assert_eq!(out[oidx + 3], 255, "a at ({dx},{dy})");
        }
    }
}
