// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! DEC Sixel decoder, `DCS q <sixel> ST` to an RGB pixel buffer.

use std::sync::Arc;

use fearless_simd::{Level, Simd};
use fearless_simd::{dispatch, prelude::*, u8x16};
use glam::{USizeVec2, UVec2};

use crate::parser::{
    Image, ImageCtx, ImageKind, ImageParser, ParseOutcome, ParsedImage,
};

/// Maximum image dimensions, to bound the decode allocation.
const MAX_DIMENSION: u32 = 4096;

/// A decoded sixel image.
#[derive(Debug, Clone)]
pub struct SixelImage {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Pixel aspect ratio numerator (`Pan` from the `"` raster attribute).
    pub aspect_x: u16,
    /// Pixel aspect ratio denominator (`Pad` from the `"` raster attribute).
    pub aspect_y: u16,
    /// RGBA pixels (`width × height × 4` bytes); α=0 for undrawn
    /// (transparent) pixels, α=255 for drawn ones.
    pub pixels: Vec<u8>,
}

/// Decode a sixel payload; `None` for an empty/invalid stream.
#[must_use]
pub fn decode_sixel(data: &[u8]) -> Option<SixelImage> {
    let mut palette = [[0_u8; 3]; 256];
    let attrs = parse_raster_attrs(data);
    // P2 mode (raster attrs give both dimensions): the bounds scan is only
    // the P1 fallback, so skip the whole-payload scan when it's unused.
    let (max_col, max_row) =
        if attrs.is_some_and(|(_, _, ph, pv)| ph > 0 && pv > 0) {
            (0, 0)
        } else {
            scan_bounds(data)?
        };
    let width = attrs
        .map(|a| a.2)
        .filter(|&w| w > 0)
        .unwrap_or(max_col)
        .min(MAX_DIMENSION);
    let height = attrs
        .map(|a| a.3)
        .filter(|&h| h > 0)
        .unwrap_or((max_row + 1) * 6)
        .min(MAX_DIMENSION);
    let (aspect_x, aspect_y) = attrs
        .filter(|&(x, y, _, _)| x > 0 && y > 0)
        .map_or((2, 1), |(x, y, _, _)| (x, y));
    let mut pixels = vec![0_u8; (width * height * 4) as usize];

    let mut color = [255_u8; 3]; // default: white
    // An undefined `#0` draws transparent pixels (the terminal background shows
    // through), as yazi's encoder expects; cache that per `#` run.
    let mut bg_is_color = false;
    let mut transparent = true;
    let mut col = 0_u32;
    let mut row = 0_u32;
    let mut i = 0;

    while i < data.len() {
        match data[i] {
            b'#' => {
                i += 1;
                let Some((n, j)) = parse_num(data, i) else {
                    break;
                };
                let idx = n.min(255) as usize;
                i = j;
                if i < data.len() && data[i] == b';' {
                    // #n;colorspace;c1;c2;c3: colorspace 1=HLS, 2=RGB.
                    let Some((_cs, j)) = parse_semicolon_num(data, i) else {
                        break;
                    };
                    let Some((c1, j)) = parse_semicolon_num(data, j) else {
                        break;
                    };
                    let Some((c2, j)) = parse_semicolon_num(data, j) else {
                        break;
                    };
                    let Some((c3, j)) = parse_semicolon_num(data, j) else {
                        break;
                    };
                    palette[idx] = [scale(c1), scale(c2), scale(c3)];
                    if idx == 0 {
                        bg_is_color = true;
                    }
                    i = j;
                }
                color = palette[idx];
                transparent = idx == 0 && !bg_is_color;
            }
            b'$' => {
                col = 0;
                i += 1;
            }
            b'-' => {
                row += 1;
                col = 0;
                i += 1;
            }
            b'!' => {
                i += 1;
                let Some((n, j)) = parse_num(data, i) else {
                    break;
                };
                i = j;
                if i < data.len() && (0x3f..=0x7e).contains(&data[i]) {
                    let s = data[i];
                    put_sixel_run(
                        &mut pixels,
                        width,
                        height,
                        col,
                        row,
                        n,
                        s,
                        color,
                        transparent,
                    );
                    col += n;
                    i += 1;
                }
            }
            0x3f..=0x7e => {
                put_sixel_run(
                    &mut pixels,
                    width,
                    height,
                    col,
                    row,
                    1,
                    data[i],
                    color,
                    transparent,
                );
                col += 1;
                i += 1;
            }
            _ => i += 1, // `"` raster attrs and separators are ignored
        }
    }

    Some(SixelImage { width, height, aspect_x, aspect_y, pixels })
}

/// Parse the first `"Pan;Pad;Ph;Pv` raster attribute (`None` when absent or
/// malformed; `Ph`/`Pv` may be zero/absent).
fn parse_raster_attrs(data: &[u8]) -> Option<(u16, u16, u32, u32)> {
    let (pan, mut i) =
        parse_num(data, data.iter().position(|&b| b == b'"')? + 1)?;
    let mut attrs = [0_u32; 3];
    for slot in &mut attrs {
        let Some((v, j)) = parse_semicolon_num(data, i) else {
            break;
        };
        *slot = v;
        i = j;
    }
    if pan == 0 || attrs[0] == 0 {
        return None;
    }
    Some((
        pan.min(65535) as u16,
        attrs[0].min(65535) as u16,
        attrs[1],
        attrs[2],
    ))
}

/// Scan the payload to find the max column and max sixel row (for P1 mode
/// where the raster attributes are absent).
fn scan_bounds(data: &[u8]) -> Option<(u32, u32)> {
    let mut col = 0_u32;
    let mut max_col = 0_u32;
    let mut row = 0_u32;
    let mut i = 0;
    while i < data.len() {
        match data[i] {
            b'$' => col = 0,
            b'-' => {
                row += 1;
                col = 0;
            }
            b'!' => {
                i += 1;
                if let Some((n, j)) = parse_num(data, i) {
                    i = j;
                    if i < data.len() && (0x3f..=0x7e).contains(&data[i]) {
                        col += n;
                        max_col = max_col.max(col);
                    }
                }
            }
            0x3f..=0x7e => {
                col += 1;
                max_col = max_col.max(col);
            }
            _ => {}
        }
        i += 1;
    }
    if max_col == 0 {
        return None;
    }
    Some((max_col, row))
}

/// Write one sixel (6 vertical pixels) across `n` columns, clamped to the
/// buffer bounds; each row is a contiguous SIMD broadcast.
#[inline]
fn put_sixel_run(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    col: u32,
    row: u32,
    n: u32,
    sixel: u8,
    color: [u8; 3],
    transparent: bool,
) {
    if col >= width || row * 6 >= height {
        return;
    }
    // Hoist the row/col base + stride out of the bit loop (the 6-iteration
    // loop stays a simple independent-branch scan that the compiler unrolls).
    let bits = sixel - 0x3f;
    let alpha = if transparent { 0 } else { 255 };
    let base = (row * 6 * width + col) as usize * 4;
    let stride = width as usize * 4;
    // The one-pixel sixel is the dominant shape: write the 4-byte RGBA
    // directly instead of routing through the SIMD span filler.
    if n == 1 {
        let rgba = [color[0], color[1], color[2], alpha];
        for bit in 0..6 {
            if bits & (1 << bit) != 0 {
                let off = base + bit * stride;
                pixels[off..off + 4].copy_from_slice(&rgba);
            }
        }
        return;
    }
    let n = n.min(width - col);
    if n == 0 {
        return;
    }
    let len = n as usize * 4;
    for bit in 0..6 {
        if bits & (1 << bit) != 0 {
            let off = base + bit * stride;
            fill_rgba(&mut pixels[off..off + len], color, alpha);
        }
    }
}

/// Broadcast-fill a contiguous RGBA span via `fearless_simd`.
#[inline]
fn fill_rgba(dst: &mut [u8], color: [u8; 3], alpha: u8) {
    let rgba = [color[0], color[1], color[2], alpha];
    // Small spans (the sixel one-pixel hot path): a scalar fill beats the
    // SIMD level probe + dispatch for a handful of bytes.
    if dst.len() <= 16 {
        for d in dst.as_chunks_mut::<4>().0 {
            d.copy_from_slice(&rgba);
        }
        return;
    }
    let level = Level::try_detect().unwrap_or_else(Level::baseline);
    dispatch!(level, simd => fill_rgba_simd(simd, dst, rgba));
}

/// Four RGBA pixels (16 bytes) per SIMD iteration; the tail falls back to a
/// scalar 4-byte copy.
#[inline(always)]
#[allow(
    clippy::chunks_exact_to_as_chunks,
    reason = "chunks_exact_mut pairs the SIMD store pattern"
)]
fn fill_rgba_simd<S: Simd>(simd: S, dst: &mut [u8], rgba: [u8; 4]) {
    let pattern = u8x16::from_slice(
        simd,
        &[
            rgba[0], rgba[1], rgba[2], rgba[3], rgba[0], rgba[1], rgba[2],
            rgba[3], rgba[0], rgba[1], rgba[2], rgba[3], rgba[0], rgba[1],
            rgba[2], rgba[3],
        ],
    );
    let mut chunks = dst.chunks_exact_mut(16);
    for d in &mut chunks {
        pattern.store_slice(d);
    }
    for d in chunks.into_remainder().as_chunks_mut::<4>().0 {
        d.copy_from_slice(&rgba);
    }
}

/// Parse a decimal number at `i`, returning `(value, next_index)`.
fn parse_num(data: &[u8], mut i: usize) -> Option<(u32, usize)> {
    let start = i;
    let mut v = 0_u32;
    while i < data.len() && data[i].is_ascii_digit() {
        v = v.saturating_mul(10).saturating_add((data[i] - b'0') as u32);
        i += 1;
    }
    (i > start).then_some((v, i))
}

/// Parse `;<number>` at `i` (the sixel color-component separator).
fn parse_semicolon_num(data: &[u8], i: usize) -> Option<(u32, usize)> {
    if i < data.len() && data[i] == b';' {
        parse_num(data, i + 1)
    } else {
        None
    }
}

/// Scale a sixel color component (0-100 percent, or 0-255 when > 100).
fn scale(v: u32) -> u8 {
    if v > 100 { v.min(255) as u8 } else { (v * 255 / 100) as u8 }
}

/// DEC Sixel (`DCS q`): stateless, the payload is the full raster.
#[derive(Default)]
pub struct SixelParser;

impl ImageParser for SixelParser {
    fn parse(
        &mut self,
        payload: &[u8],
        final_byte: u8,
        ctx: ImageCtx,
    ) -> ParseOutcome {
        // Sixel and legacy kitty graphics share `DCS q`; kitty headers always
        // contain `=`, sixel data never does.
        if final_byte != b'q' || payload.contains(&b'=') {
            return ParseOutcome::Skip;
        }
        let Some(img) = decode_sixel(payload) else {
            return ParseOutcome::Skip;
        };
        // `decode_sixel` already emits RGBA with α=0 for undrawn pixels, so
        // transparent backgrounds survive instead of becoming opaque black.
        let rgba = img.pixels;
        let viewport_row = ctx.cursor_row.saturating_sub(1);
        ParseOutcome::Handled(ParsedImage {
            image: Some(Image {
                kind: ImageKind::Sixel,
                id: 0,
                version: crate::parser::next_version(),
                anchor: USizeVec2::new(
                    ctx.cursor_col.saturating_sub(1),
                    ctx.screen_start + viewport_row,
                ),
                cell_size: UVec2::ZERO,
                size: UVec2::new(img.width, img.height),
                cell_offset: UVec2::ZERO,
                z_index: 0,
                aspect: UVec2::new(img.aspect_x as u32, img.aspect_y as u32),
                pixels: Arc::from(rgba),
            }),
            reply: None,
            advance_cursor: false,
            clear_image: false,
            delete_id: None,
            delete_image_id: None,
        })
    }
}

#[cfg(test)]
#[path = "../tests/sixel/mod.rs"]
mod tests;
