// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::decode_sixel;

#[test]
fn decodes_simple_image() {
    // `#0;2;100;0;0` defines color 0 as red (100% r), then `~` (0x7e)
    // draws all 6 vertical pixels → a 1×6 red line.
    let img = decode_sixel(b"#0;2;100;0;0~").unwrap();
    assert_eq!((img.width, img.height), (1, 6));
    assert_eq!(img.pixels.len(), 24);
    for px in img.pixels.chunks(4) {
        assert_eq!(px, [255, 0, 0, 255]);
    }
}

#[test]
fn clamps_oversized_dimensions() {
    // A maliciously wide image is clipped to MAX_DIMENSION (4096).
    let img = decode_sixel(b"#0;2;100;0;0!5000~").unwrap();
    assert_eq!(img.width, 4096);
}

#[test]
fn decodes_repeat_and_newline() {
    // Red and green palette entries, then `~!60~-!60~-` draws 61 green sixels
    // on the first row and 60 on the second.
    let img = decode_sixel(b"#0;2;100;0;0#1;2;0;100;0~!60~-!60~-").unwrap();
    assert_eq!((img.width, img.height), (61, 18));
    // Top-left is green; the bottom-right cell (col 60, second row) is
    // undrawn (transparent) because the second row has only 60 sixels.
    assert_eq!(&img.pixels[0..4], &[0, 255, 0, 255]);
    let last = (17 * 61 + 60) * 4;
    assert_eq!(&img.pixels[last..last + 4], &[0, 0, 0, 0]);
}

#[test]
fn honors_raster_attributes() {
    // `"1;1;4;6` declares a 4×6 image with square (1:1) pixels, larger
    // than the actual `~` data (one sixel), so the extra columns are black.
    let img = decode_sixel(b"\"1;1;4;6#0;2;100;0;0~").unwrap();
    assert_eq!((img.width, img.height), (4, 6));
    assert_eq!((img.aspect_x, img.aspect_y), (1, 1));
    // Col 0 rows 0..5 red; col 1 is transparent (declared width padding).
    assert_eq!(&img.pixels[0..4], &[255, 0, 0, 255]);
    assert_eq!(&img.pixels[4..8], &[0, 0, 0, 0]);
}

#[test]
fn undefined_color_zero_is_transparent() {
    // yazi-style sixel: no `#0` definition, the transparent background is
    // drawn with color 0. Drawing only `#0` sixels must yield α=0.
    let img = decode_sixel(b"#1;2;100;0;0#0~").unwrap();
    assert_eq!((img.width, img.height), (1, 6));
    for px in img.pixels.chunks(4) {
        assert_eq!(px, [0, 0, 0, 0]);
    }
}

#[test]
fn p2_raster_attrs_skip_the_bounds_scan() {
    // `"1;1;4;6` gives width=4, height=6 (P2); the payload draws no pixels, so
    // the P2 path must not require the P1 bounds scan.
    let img = decode_sixel(b"\"1;1;4;6").unwrap();
    assert_eq!((img.width, img.height), (4, 6));
    assert_eq!((img.aspect_x, img.aspect_y), (1, 1));
}
