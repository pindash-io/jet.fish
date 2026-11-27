// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Kitty payload decoding: base64 → raw RGB/RGBA or PNG.

use compact_str::CompactString;

use crate::MAX_IMAGE_PAYLOAD;

/// Decode kitty image data to RGBA pixels: base64 → raw RGB/RGBA (f=24/32) or
/// PNG (f=100).
#[must_use]
pub fn decode_kitty_rgba(
    data: &[u8],
    format: u8,
    width: u32,
    height: u32,
    compressed: bool,
) -> Option<(u32, u32, Vec<u8>)> {
    use bases::Base64;
    let decoded = Base64::decode(data).ok()?;
    let decoded =
        if compressed { zlib_decompress(&decoded)? } else { decoded };
    decode_kitty_raw(&decoded, format, width, height)
}

/// Decode raw kitty image bytes (base64 already decoded) to RGBA. f=24 is
/// RGB → RGBA (α=255), f=32 is RGBA, f=100 is PNG.
#[must_use]
pub fn decode_kitty_raw(
    data: &[u8],
    format: u8,
    width: u32,
    height: u32,
) -> Option<(u32, u32, Vec<u8>)> {
    let decoded = match format {
        24 => {
            let w = width.max(1);
            let h = height.max(1);
            let px = (w as usize).checked_mul(h as usize)?;
            let want_rgb = px.checked_mul(3)?;
            // Reject absurd declared dimensions: the payload is capped, but a hostile
            // `s=`/`v=` header could still drive a huge allocation.
            if want_rgb > MAX_IMAGE_PAYLOAD {
                return None;
            }
            let mut rgb = data.to_vec();
            rgb.truncate(want_rgb);
            rgb.resize(want_rgb, 0);
            let rgba = crate::rgb_to_rgba(&rgb, w, h)?;
            (w, h, rgba)
        }
        32 => {
            let px = (width as usize).checked_mul(height as usize)?;
            let want = px.checked_mul(4)?;
            if want > MAX_IMAGE_PAYLOAD {
                return None;
            }
            let mut out = data.to_vec();
            out.truncate(want);
            out.resize(want, 0);
            (width.max(1), height.max(1), out)
        }
        100 => crate::decode_image(data)?,
        _ => return None,
    };
    Some(decoded)
}

/// Decode a kitty image by file path: the payload is the base64-encoded path.
#[must_use]
pub fn decode_kitty_file(
    data: &[u8],
    format: u8,
    width: u32,
    height: u32,
) -> Option<(u32, u32, Vec<u8>)> {
    use bases::Base64;
    let path_bytes = Base64::decode(data).ok()?;
    let path = CompactString::from_utf8(&path_bytes).ok()?;
    let bytes = std::fs::read(path.as_str()).ok()?;
    decode_kitty_raw(&bytes, format, width, height)
}

/// Inflate a zlib (RFC 1950) stream.
fn zlib_decompress(bytes: &[u8]) -> Option<Vec<u8>> {
    use std::io::Read as _;
    let mut out = Vec::new();
    flate2::read::ZlibDecoder::new(bytes).read_to_end(&mut out).ok()?;
    Some(out)
}

/// Crop an RGBA raster to a source rectangle (`x`, `y`, `w`, `h`); `w=0` or
/// `h=0` means "to the edge". Clamped to the raster.
pub(super) fn crop_rgba(
    rgba: &[u8],
    w: u32,
    h: u32,
    x: u32,
    y: u32,
    cw: u32,
    ch: u32,
) -> (u32, u32, Vec<u8>) {
    if x >= w || y >= h || rgba.is_empty() {
        return (0, 0, Vec::new());
    }
    let cw = if cw == 0 || x.saturating_add(cw) > w { w - x } else { cw };
    let ch = if ch == 0 || y.saturating_add(ch) > h { h - y } else { ch };
    let (w, x, y, cw, ch) =
        (w as usize, x as usize, y as usize, cw as usize, ch as usize);
    let mut out = Vec::with_capacity(cw.saturating_mul(ch).saturating_mul(4));
    for row in y..y + ch {
        let start = (row * w + x) * 4;
        let end = start + cw * 4;
        if end <= rgba.len() {
            out.extend_from_slice(&rgba[start..end]);
        }
    }
    (cw as u32, ch as u32, out)
}

#[cfg(test)]
#[path = "../../tests/kitty_parser/mod.rs"]
mod parser_tests;
