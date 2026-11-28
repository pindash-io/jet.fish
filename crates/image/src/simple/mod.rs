// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Dependency-light PNG decode and RGBA resize (`simple` feature): a hand-rolled
//! `png` decoder with SIMD color expansion and a premultiplied box downscale.

#![allow(
    clippy::chunks_exact_to_as_chunks,
    reason = "chunks_exact + zip pairs two SIMD iterators; as_chunks forces tuple/remainder handling"
)]

use fearless_simd::{Level, Simd};
use fearless_simd::{dispatch, prelude::*, u8x16};

mod resize;

pub use resize::scale_down_rgba;

/// Expand packed RGB (3 bytes/px) into RGBA (4 bytes/px) with `α = 255`.
#[inline]
pub(crate) fn rgb_to_rgba(rgb: &[u8], rgba: &mut [u8]) {
    debug_assert_eq!(
        rgb.len() / 3 * 4,
        rgba.len(),
        "rgba must be 4 bytes per RGB triplet"
    );
    let level = Level::try_detect().unwrap_or(Level::baseline());
    dispatch!(level, simd => rgb_to_rgba_simd(simd, rgb, rgba));
}

/// Four pixels (12 RGB bytes → 16 RGBA bytes) per SIMD iteration via a
/// byte-table shuffle; the `< 4`-pixel tail falls back to scalar.
#[inline(always)]
fn rgb_to_rgba_simd<S: Simd>(simd: S, rgb: &[u8], rgba: &mut [u8]) {
    let indices = u8x16::from_slice(
        simd,
        &[0, 1, 2, 15, 3, 4, 5, 15, 6, 7, 8, 15, 9, 10, 11, 15],
    );
    let mut src = rgb.chunks_exact(12);
    let mut dst = rgba.chunks_exact_mut(16);
    for (s, d) in (&mut src).zip(&mut dst) {
        let mut buf = [0_u8; 16];
        buf[..12].copy_from_slice(s);
        buf[15] = 255;
        let v = u8x16::from_slice(simd, &buf);
        simd.swizzle_dyn_u8x16(v, indices).store_slice(d);
    }
    for (o, i) in dst
        .into_remainder()
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(src.remainder().chunks_exact(3))
    {
        o[..3].copy_from_slice(i);
        o[3] = 255;
    }
}

/// Expand packed grayscale (1 byte/px) into RGBA (4 bytes/px) with `α = 255`.
#[inline]
pub(crate) fn gray_to_rgba(gray: &[u8], rgba: &mut [u8]) {
    debug_assert_eq!(
        gray.len() * 4,
        rgba.len(),
        "rgba must be 4 bytes per gray byte"
    );
    let level = Level::try_detect().unwrap_or(Level::baseline());
    dispatch!(level, simd => gray_to_rgba_simd(simd, gray, rgba));
}

/// Four grayscale pixels (4 bytes) → 16 RGBA bytes per SIMD iteration.
#[inline(always)]
fn gray_to_rgba_simd<S: Simd>(simd: S, gray: &[u8], rgba: &mut [u8]) {
    let indices = u8x16::from_slice(
        simd,
        &[0, 0, 0, 15, 1, 1, 1, 15, 2, 2, 2, 15, 3, 3, 3, 15],
    );
    let mut src = gray.chunks_exact(4);
    let mut dst = rgba.chunks_exact_mut(16);
    for (s, d) in (&mut src).zip(&mut dst) {
        let mut buf = [0_u8; 16];
        buf[..4].copy_from_slice(s);
        buf[15] = 255;
        let v = u8x16::from_slice(simd, &buf);
        simd.swizzle_dyn_u8x16(v, indices).store_slice(d);
    }
    for (o, gray) in dst
        .into_remainder()
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(src.remainder())
    {
        o[..3].fill(*gray);
        o[3] = 255;
    }
}

/// Expand packed grayscale+alpha (2 bytes/px) into RGBA (4 bytes/px).
#[inline]
pub(crate) fn gray_alpha_to_rgba(ga: &[u8], rgba: &mut [u8]) {
    debug_assert_eq!(
        ga.len() / 2 * 4,
        rgba.len(),
        "rgba must be 4 bytes per gray-alpha pair"
    );
    let level = Level::try_detect().unwrap_or(Level::baseline());
    dispatch!(level, simd => gray_alpha_to_rgba_simd(simd, ga, rgba));
}

/// Four grayscale+alpha pixels (8 bytes) → 16 RGBA bytes per SIMD iteration.
#[inline(always)]
fn gray_alpha_to_rgba_simd<S: Simd>(simd: S, ga: &[u8], rgba: &mut [u8]) {
    let indices = u8x16::from_slice(
        simd,
        &[0, 0, 0, 1, 2, 2, 2, 3, 4, 4, 4, 5, 6, 6, 6, 7],
    );
    let mut src = ga.chunks_exact(8);
    let mut dst = rgba.chunks_exact_mut(16);
    for (s, d) in (&mut src).zip(&mut dst) {
        let mut buf = [0_u8; 16];
        buf[..8].copy_from_slice(s);
        let v = u8x16::from_slice(simd, &buf);
        simd.swizzle_dyn_u8x16(v, indices).store_slice(d);
    }
    for (o, i) in dst
        .into_remainder()
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(src.remainder().chunks_exact(2))
    {
        o[..3].fill(i[0]);
        o[3] = i[1];
    }
}

/// Decode a PNG byte stream to `(width, height, RGBA8)` using the `png` crate
/// (kitten `icat` and iTerm2 inline images transmit PNG).
pub(crate) fn decode_png(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    // Normalize palettes/tRNS/16-bit so only 8-bit RGB/RGBA remain.
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().ok()?;
    let size = reader.output_buffer_size()?;
    if size > crate::MAX_IMAGE_PAYLOAD {
        return None;
    }
    let mut buf = vec![0; size];
    let info = reader.next_frame(&mut buf).ok()?;
    let (w, h) = (info.width, info.height);
    let raw = &buf[..info.buffer_size()];
    let px = (w as usize).checked_mul(h as usize)?;
    let mut rgba = vec![0; px * 4];
    match info.color_type {
        png::ColorType::Rgb => rgb_to_rgba(raw, &mut rgba),
        png::ColorType::Rgba => rgba.copy_from_slice(raw),
        png::ColorType::Grayscale => gray_to_rgba(raw, &mut rgba),
        png::ColorType::GrayscaleAlpha => gray_alpha_to_rgba(raw, &mut rgba),
        _ => return None,
    }
    Some((w, h, rgba))
}

#[cfg(test)]
#[path = "../../tests/simple/mod.rs"]
mod tests;
