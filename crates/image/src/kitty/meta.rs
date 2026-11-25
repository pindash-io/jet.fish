// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Kitty graphics control header (`key=value` pairs) parsing.

use compact_str::CompactString;
use glam::UVec2;

use super::decode::{decode_kitty_file, decode_kitty_rgba};

/// A parsed kitty graphics control header (`DCS q` / `APC _G`) — the
/// `key=value` pairs before the `;` that separates the payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KittyMeta {
    /// Image id (`i=`).
    pub id: u32,
    /// Transmission action (`a=`), one of `t`, `T`, `p`, `P`, `q`, `f`.
    pub action: char,
    /// Pixel size (`s=`/`v=`).
    pub size: UVec2,
    /// Cell size (`c=`/`r=`) — the cell-dimension form of `s=`/`v=`.
    pub cell_size: UVec2,
    /// Source-rect origin (`x=`/`y=`) — crop the image before display.
    pub src: UVec2,
    /// Source-rect size (`w=`/`h=`) — 0 means the full image.
    pub src_size: UVec2,
    /// Pixel format (`f=`): 24 = RGB, 32 = RGBA, 100 = PNG.
    pub format: u8,
    /// Transmission medium (`t=`): `d` = direct (base64 in the payload),
    /// `f`/`t`/`s` = file / temp file / shared memory.
    pub medium: char,
    /// Compression (`o=`): `z` = zlib-compressed payload (RFC 1950).
    pub compression: char,
    /// Whether more chunks follow (`m=1`): the payload is appended to the
    /// previous chunk's base64 instead of decoded as a complete image.
    pub more: bool,
    /// Quiet flag (`q=`): 1 suppresses OK responses, 2 suppresses all
    /// responses. It does NOT gate display (that is the `t` vs `T` action).
    pub quiet: u8,
    /// Placement id (`p=`) — uniquely identifies a placement for
    /// acknowledgements and move/replace.
    pub placement_id: u32,
    /// Cursor movement policy (`C=`): `false` (`C=1`) = leave the cursor;
    /// `true` (default `C=0`) = move right `c` / down `r-1` after placement.
    pub cursor_move: bool,
    /// Pixel offset within the starting cell (`X=`/`Y=`, uppercase).
    pub cell_offset: UVec2,
    /// z-index (`z=`) — negative draws the image under the text.
    pub z_index: i32,
    /// Delete specifier (`d=`) for `a=d`: `a`/`A` = all visible, `i`/`I` =
    /// the image id.
    pub delete_spec: char,
    /// Unicode placeholder (`U=1`): the image is placed through the
    /// U+10EEEE placeholder cells instead of the cursor anchor.
    pub unicode_placeholder: bool,
}

impl KittyMeta {
    /// An empty header (all defaults).
    pub const fn new() -> Self {
        Self {
            id: 0,
            action: 't',
            size: UVec2::ZERO,
            cell_size: UVec2::ZERO,
            src: UVec2::ZERO,
            src_size: UVec2::ZERO,
            format: 24,
            medium: 'd',
            compression: ' ',
            more: false,
            quiet: 0,
            placement_id: 0,
            cursor_move: true,
            cell_offset: UVec2::ZERO,
            z_index: 0,
            delete_spec: 'a',
            unicode_placeholder: false,
        }
    }

    /// Decode this header's payload depending on the transmission medium:
    /// `d` = direct base64 (with optional `o=z` zlib), `f`/`t` = a
    /// base64-encoded file path.
    pub fn decode(&self, base64: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
        match self.medium {
            'd' => decode_kitty_rgba(
                base64,
                self.format,
                self.size.x,
                self.size.y,
                self.compression == 'z',
            ),
            'f' | 't' => decode_kitty_file(
                base64,
                self.format,
                self.size.x,
                self.size.y,
            ),
            _ => None,
        }
    }
}

impl Default for KittyMeta {
    fn default() -> Self {
        Self::new()
    }
}

/// A decoded kitty image (`DCS q` / `APC _G`): the control header plus the
/// base64-decoded payload bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KittyImage {
    /// Control header (`a=`, `f=`, `s=`, `v=`, `i=`, …).
    pub meta: KittyMeta,
    /// Decoded payload bytes: raw RGB/RGBA (f=24/32) or PNG (f=100).
    pub pixels: Vec<u8>,
}

/// Parse an ASCII decimal integer from bytes.
fn parse_num<T: core::str::FromStr>(v: &[u8]) -> Option<T> {
    CompactString::from_utf8(v).ok()?.parse().ok()
}

/// The first byte as a `char`.
#[inline]
fn first_char(v: &[u8]) -> Option<char> {
    v.first().copied().map(char::from)
}

/// Parse a kitty graphics header; returns the parsed header plus the raw
/// (still-base64) payload.
#[must_use]
pub fn parse_kitty_meta(payload: &[u8]) -> Option<(KittyMeta, &[u8])> {
    let mut parts = payload.splitn(2, |&b| b == b';');
    let header = parts.next().unwrap_or_default();
    let data = parts.next().unwrap_or(&[]);

    let mut img = KittyMeta::new();
    for pair in header.split(|&b| b == b',') {
        let mut kv = pair.splitn(2, |&b| b == b'=');
        let (Some(k), Some(v)) = (kv.next(), kv.next()) else {
            continue;
        };
        match k {
            b"a" => img.action = first_char(v).unwrap_or(img.action),
            b"i" => img.id = parse_num(v).unwrap_or_default(),
            b"s" => img.size.x = parse_num(v).unwrap_or_default(),
            b"v" => img.size.y = parse_num(v).unwrap_or_default(),
            // `c`/`r` are the cell-dimension form of `s`/`v` (pixels).
            // jcode and other clients send cells instead of pixels, so
            // both must parse or the size stays 0 for those clients.
            b"c" => img.cell_size.x = parse_num(v).unwrap_or_default(),
            b"r" => img.cell_size.y = parse_num(v).unwrap_or_default(),
            // Source rect (which part of the image to display) — used
            // when the image is cropped to fit the space below the cursor.
            b"x" => img.src.x = parse_num(v).unwrap_or_default(),
            b"y" => img.src.y = parse_num(v).unwrap_or_default(),
            b"w" => img.src_size.x = parse_num(v).unwrap_or_default(),
            b"h" => img.src_size.y = parse_num(v).unwrap_or_default(),
            b"f" => img.format = parse_num(v).unwrap_or(img.format),
            // Transmission medium (`t=`): d=direct, f/t/s=file/temp/shm.
            b"t" => img.medium = first_char(v).unwrap_or(img.medium),
            // Compression (`o=`): z = zlib (RFC 1950).
            b"o" => img.compression = first_char(v).unwrap_or(img.compression),
            // Chunking: `m=1` = more chunks follow (append, don't decode),
            // `m=0`/absent = final chunk (decode the accumulated base64).
            b"m" => img.more = parse_num::<u8>(v).is_some_and(|n| n != 0),
            // Quiet/placement (`q=`): 1 = suppress OK responses,
            // 2 = suppress all responses. Display is gated by `t` vs `T`.
            b"q" => img.quiet = parse_num(v).unwrap_or(img.quiet),
            // Placement id (`p=`) — for acknowledgements and move/replace.
            b"p" => {
                img.placement_id = parse_num(v).unwrap_or(img.placement_id);
            }
            // Cursor movement policy (`C=`): `C=1` leaves the cursor.
            b"C" if let Some(n) = parse_num::<u8>(v) => {
                img.cursor_move = n != 1;
            }
            // Pixel offset within the starting cell (`X=`/`Y=`, uppercase).
            b"X" => {
                img.cell_offset.x = parse_num(v).unwrap_or(img.cell_offset.x);
            }
            b"Y" => {
                img.cell_offset.y = parse_num(v).unwrap_or(img.cell_offset.y);
            }
            // z-index (`z=`) — negative draws under the text.
            b"z" => img.z_index = parse_num(v).unwrap_or(img.z_index),
            // Delete specifier (`d=`) for `a=d`.
            b"d" => img.delete_spec = first_char(v).unwrap_or(img.delete_spec),
            // Unicode placeholder (`U=1`): place through the U+10EEEE
            // placeholder cells instead of the cursor anchor.
            b"U" => {
                img.unicode_placeholder =
                    parse_num::<u8>(v).is_some_and(|n| n != 0);
            }
            _ => {}
        }
    }
    Some((img, data))
}
