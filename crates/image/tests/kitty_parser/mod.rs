// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

use bases::Base64;
use glam::UVec2;

use super::crop_rgba;
use crate::kitty::KittyParser;
use crate::parser::{ImageCtx, ImageParser, ParseOutcome, ParsedImage};

#[test]
fn crop_rgba_clamps_overflowing_extent() {
    // 2×2 RGBA.
    let rgba = [
        255_u8, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
    ];
    // x=1, w=2, disp_w=u32::MAX: `x + disp_w` would overflow u32 without
    // the saturating clamp; the crop must clamp to the raster edge.
    let (cw, ch, out) = crop_rgba(&rgba, 2, 2, 1, 0, u32::MAX, 0);
    assert_eq!((cw, ch), (1, 2));
    assert_eq!(out.len(), 2 * 4);
}

#[test]
fn kitty_file_transmission_decodes() {
    // broot's default medium: `t=t` with a base64 temp-file path holding
    // raw RGBA. Write the file, then drive the parser.
    let dir = std::env::temp_dir();
    let path = dir.join("jetfish_kitty_file_test.rgba");
    let rgba = [
        255_u8, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
    ];
    std::fs::write(&path, rgba).unwrap();
    let b64 = Base64::encode(path.to_str().unwrap().as_bytes());
    let payload = format!("a=T,f=32,t=t,i=1,s=2,v=2;{b64}");

    let mut parser = KittyParser::default();
    let ctx = ImageCtx { cursor_col: 1, cursor_row: 1, screen_start: 0 };
    match parser.parse(payload.as_bytes(), b'G', ctx) {
        ParseOutcome::Handled(ParsedImage { image: Some(img), .. }) => {
            assert_eq!(img.size, UVec2::new(2, 2));
            assert_eq!(img.pixels.as_ref(), &rgba[..]);
        }
        other => panic!("unexpected {other:?}"),
    }
    std::fs::remove_file(&path).unwrap();
}
