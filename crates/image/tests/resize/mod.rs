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
    assert_eq!(&out[..3], &[128, 128, 128]);
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
