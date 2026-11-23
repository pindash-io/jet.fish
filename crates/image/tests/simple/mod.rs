// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::{decode_png, gray_alpha_to_rgba, gray_to_rgba, rgb_to_rgba};

fn xorshift_bytes() -> impl FnMut() -> u8 {
    let mut state = 0x9E37_79B9_7F4A_7C15_u64;
    move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state & 0xFF) as u8
    }
}

#[test]
fn rgb_to_rgba_expands_and_pads() {
    for n in [0, 1, 2, 3, 4, 5, 16, 17, 100] {
        let rgb: Vec<u8> = (0..n * 3).map(|i| (i % 251) as u8).collect();
        let mut rgba = vec![0_u8; n * 4];
        rgb_to_rgba(&rgb, &mut rgba);
        for (i, px) in rgba.as_chunks::<4>().0.iter().enumerate() {
            assert_eq!(&px[..3], &rgb[i * 3..i * 3 + 3]);
            assert_eq!(px[3], 255);
        }
    }
}

#[test]
fn rgb_to_rgba_simd_matches_scalar() {
    // Deterministic xorshift64 byte stream.
    let mut state = 0x9E37_79B9_7F4A_7C15_u64;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state & 0xFF) as u8
    };
    // Every tail length, then larger sizes that run the SIMD main loop
    // many times.
    let mut sizes: Vec<usize> = (0..=20).collect();
    sizes.extend([64, 256, 1024, 4096, 100_000]);
    for n in sizes {
        let rgb: Vec<u8> = (0..n * 3).map(|_| next()).collect();
        let mut simd_out = vec![0_u8; n * 4];
        let mut scalar_out = vec![0_u8; n * 4];
        rgb_to_rgba(&rgb, &mut simd_out);
        for (o, i) in scalar_out
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(rgb.chunks_exact(3))
        {
            o[..3].copy_from_slice(i);
            o[3] = 255;
        }
        assert_eq!(simd_out, scalar_out, "RGB→RGBA mismatch at {n} px");
    }
}

#[test]
fn rgb_to_rgba_simd_matches_scalar_on_cowboy() {
    // Real-world data: cowboy.png's RGB channels.
    let png = include_bytes!("../fixtures/cowboy.png");
    let (_, _, rgba) = decode_png(png).unwrap();
    let mut rgb = Vec::with_capacity(rgba.len() / 4 * 3);
    for px in rgba.chunks(4) {
        rgb.extend_from_slice(&px[..3]);
    }
    let mut simd_out = vec![0_u8; rgb.len() / 3 * 4];
    let mut scalar_out = vec![0_u8; rgb.len() / 3 * 4];
    rgb_to_rgba(&rgb, &mut simd_out);
    for (o, i) in
        scalar_out.as_chunks_mut::<4>().0.iter_mut().zip(rgb.chunks_exact(3))
    {
        o[..3].copy_from_slice(i);
        o[3] = 255;
    }
    assert_eq!(simd_out, scalar_out, "RGB→RGBA mismatch on cowboy");
}

#[test]
fn gray_to_rgba_simd_matches_scalar() {
    let mut next = xorshift_bytes();
    for n in [0, 1, 2, 3, 4, 5, 7, 11, 16, 100, 4096, 100_000] {
        let gray: Vec<u8> = (0..n).map(|_| next()).collect();
        let mut simd_out = vec![0_u8; n * 4];
        let mut scalar_out = vec![0_u8; n * 4];
        gray_to_rgba(&gray, &mut simd_out);
        for (o, g) in scalar_out.as_chunks_mut::<4>().0.iter_mut().zip(&gray) {
            o[..3].fill(*g);
            o[3] = 255;
        }
        assert_eq!(simd_out, scalar_out, "gray→RGBA mismatch at {n} px");
    }
}

#[test]
fn gray_alpha_to_rgba_simd_matches_scalar() {
    let mut next = xorshift_bytes();
    for n in [0, 1, 2, 3, 4, 5, 7, 11, 16, 100, 4096, 100_000] {
        let ga: Vec<u8> = (0..n * 2).map(|_| next()).collect();
        let mut simd_out = vec![0_u8; n * 4];
        let mut scalar_out = vec![0_u8; n * 4];
        gray_alpha_to_rgba(&ga, &mut simd_out);
        for (o, i) in scalar_out
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(ga.chunks_exact(2))
        {
            o[..3].fill(i[0]);
            o[3] = i[1];
        }
        assert_eq!(simd_out, scalar_out, "gray+α→RGBA mismatch at {n} px");
    }
}

#[test]
fn decode_png_preserves_transparent_background() {
    // cowboy.png is an RGBA sprite with a transparent background; the
    // transparent pixels must decode as α=0, not opaque black.
    let png = include_bytes!("../fixtures/cowboy.png");
    let (w, h, rgba) = decode_png(png).unwrap();
    assert_eq!((w, h), (80, 80));
    assert_eq!(&rgba[0..4], &[0, 0, 0, 0], "corner is transparent");
    let transparent = rgba.chunks(4).filter(|px| px[3] == 0).count();
    assert_eq!(transparent, 2370, "transparent pixels preserved");
}

#[test]
fn decode_png_grayscale_and_alpha() {
    // 2×1 grayscale+alpha: gray is replicated into RGB, alpha preserved.
    let mut data = Vec::new();
    {
        let mut enc = png::Encoder::new(std::io::Cursor::new(&mut data), 2, 1);
        enc.set_color(png::ColorType::GrayscaleAlpha);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc.write_header().unwrap();
        writer.write_image_data(&[0, 255, 128, 0]).unwrap();
    }
    let (w, h, rgba) = decode_png(&data).unwrap();
    assert_eq!((w, h), (2, 1));
    assert_eq!(rgba, [0, 0, 0, 255, 128, 128, 128, 0]);
}
