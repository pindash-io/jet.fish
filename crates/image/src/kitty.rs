// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Kitty graphics protocol image parsing (`DCS q` / `APC _G`).

use std::collections::VecDeque;
use std::fmt::Write as _;
use std::sync::Arc;

use compact_str::CompactString;
use glam::{USizeVec2, UVec2};
use hashbrown::HashMap;

use crate::MAX_IMAGE_PAYLOAD;
use crate::parser::{
    Image, ImageCtx, ImageKind, ImageParser, ParseOutcome, ParsedImage,
    VirtualPlacement,
};

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

/// Decode RAW kitty image bytes (base64 already decoded) to RGBA. f=24 is
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
            // Reject absurdly large declared dimensions so a hostile `s=`/`v=`
            // header can't drive a huge allocation (the payload is capped,
            // but the declared size is not).
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

#[cfg(test)]
#[path = "../tests/kitty/mod.rs"]
mod tests;

/// Kitty graphics (`DCS q` legacy / `APC _G` modern) — owns the stored-image
/// map and the `m=` chunk-reassembly buffer.
#[derive(Default)]
pub struct KittyParser {
    kitty_images: HashMap<u32, (KittyMeta, Vec<u8>)>,
    kitty_order: VecDeque<u32>,
    kitty_bytes: usize,
    kitty_pending: Option<(KittyMeta, Vec<u8>)>,
    /// `a=p,U=1` virtual placements, keyed by placement id (`p=`).
    virtual_placements: HashMap<u32, VirtualPlacement>,
    /// The most recent placement id per image id.
    latest_placements: HashMap<u32, u32>,
}

impl KittyParser {
    /// Insert into the upload cache (the still-base64 payload), evicting the
    /// oldest entries (FIFO) once the total stored bytes exceed
    /// `MAX_IMAGE_PAYLOAD × 4`. Decode is deferred to the `p`/`P`/`materialize`
    /// placement so a transmit-then-never-place stream never pays the base64
    /// + image decode (the `kitty` corpus).
    fn cache_insert(&mut self, meta: KittyMeta, base64: Vec<u8>) {
        let id = meta.id;
        let size = base64.len();
        if let Some((_, old)) = self.kitty_images.insert(id, (meta, base64)) {
            self.kitty_bytes = self.kitty_bytes.saturating_sub(old.len());
        } else {
            self.kitty_order.push_back(id);
        }
        self.kitty_bytes = self.kitty_bytes.saturating_add(size);
        const CAP: usize = MAX_IMAGE_PAYLOAD * 4;
        while self.kitty_bytes > CAP {
            let Some(id) = self.kitty_order.pop_front() else {
                break;
            };
            if let Some((_, base64)) = self.kitty_images.remove(&id) {
                self.kitty_bytes =
                    self.kitty_bytes.saturating_sub(base64.len());
            }
        }
    }

    /// Store, place, or probe depending on the transmission action.
    fn place(
        &mut self,
        meta: KittyMeta,
        base64: Vec<u8>,
        ctx: ImageCtx,
    ) -> ParseOutcome {
        match meta.action {
            // Transmit only: store the still-base64 payload, no display.
            // Decode is deferred to `p`/`P`/`materialize` (a transmit-then-
            // never-place stream pays no decode).
            't' => {
                self.cache_insert(meta.clone(), base64);
                ParseOutcome::Handled(ParsedImage {
                    image: None,
                    reply: Some(self.ack(&meta, true)),
                    advance_cursor: false,
                    clear_image: false,
                    delete_id: None,
                    delete_image_id: None,
                })
            }
            // Transmit + display at the cursor.
            'T' => {
                let Some((w, h, rgba)) = meta.decode(&base64) else {
                    return ParseOutcome::Handled(ParsedImage {
                        image: None,
                        reply: Some(self.ack(&meta, false)),
                        advance_cursor: false,
                        clear_image: false,
                        delete_id: None,
                        delete_image_id: None,
                    });
                };
                self.cache_insert(meta.clone(), base64);
                if meta.unicode_placeholder {
                    self.store_virtual(&meta);
                    return ParseOutcome::Handled(ParsedImage {
                        image: None,
                        reply: Some(self.ack(&meta, true)),
                        advance_cursor: false,
                        clear_image: false,
                        delete_id: None,
                        delete_image_id: None,
                    });
                }
                let image = self.display(&meta, w, h, &rgba, ctx);
                ParseOutcome::Handled(ParsedImage {
                    image: Some(image),
                    reply: Some(self.ack(&meta, true)),
                    advance_cursor: meta.cursor_move
                        && (meta.cell_size.x > 0 || meta.cell_size.y > 0),
                    clear_image: false,
                    delete_id: None,
                    delete_image_id: None,
                })
            }
            // Place a previously transmitted image at the cursor.
            'p' | 'P' => {
                if meta.unicode_placeholder {
                    self.store_virtual(&meta);
                    return ParseOutcome::Handled(ParsedImage {
                        image: None,
                        reply: Some(self.ack(&meta, true)),
                        advance_cursor: false,
                        clear_image: false,
                        delete_id: None,
                        delete_image_id: None,
                    });
                }
                match self.kitty_images.get(&meta.id).cloned() {
                    Some((stored, base64)) => {
                        let Some((w, h, rgba)) = stored.decode(&base64) else {
                            return ParseOutcome::Handled(ParsedImage {
                                image: None,
                                reply: Some(self.ack(&meta, false)),
                                advance_cursor: false,
                                clear_image: false,
                                delete_id: None,
                                delete_image_id: None,
                            });
                        };
                        let image = self.display(&meta, w, h, &rgba, ctx);
                        ParseOutcome::Handled(ParsedImage {
                            image: Some(image),
                            reply: Some(self.ack(&meta, true)),
                            advance_cursor: meta.cursor_move
                                && (meta.cell_size.x > 0
                                    || meta.cell_size.y > 0),
                            clear_image: false,
                            delete_id: None,
                            delete_image_id: None,
                        })
                    }
                    None => ParseOutcome::Handled(ParsedImage {
                        image: None,
                        reply: Some(self.ack(&meta, false)),
                        advance_cursor: false,
                        clear_image: false,
                        delete_id: None,
                        delete_image_id: None,
                    }),
                }
            }
            // Capability query: reply OK for a supported medium + format.
            'q' => {
                let ok = matches!(meta.medium, 'd' | 'f' | 't')
                    && matches!(meta.format, 24 | 32 | 100);
                ParseOutcome::Handled(ParsedImage {
                    image: None,
                    reply: Some(self.ack(&meta, ok)),
                    advance_cursor: false,
                    clear_image: false,
                    delete_id: None,
                    delete_image_id: None,
                })
            }
            // Delete (`a=d`). kitty distinguishes lowercase (delete the
            // placements only, keep the upload cache) from uppercase (also
            // free the image data).
            'd' => {
                let free_images = meta.delete_spec.is_ascii_uppercase();
                let (clear_all, delete_id, delete_image_id) =
                    match meta.delete_spec {
                        'i' | 'I' => {
                            if free_images {
                                if let Some((_, base64)) =
                                    self.kitty_images.remove(&meta.id)
                                {
                                    self.kitty_bytes = self
                                        .kitty_bytes
                                        .saturating_sub(base64.len());
                                }
                                self.kitty_order.retain(|&id| id != meta.id);
                            }
                            self.virtual_placements
                                .retain(|_, vp| vp.image_id != meta.id);
                            self.latest_placements.remove(&meta.id);
                            let placement = (meta.placement_id != 0)
                                .then_some(meta.placement_id);
                            (false, placement, Some(meta.id))
                        }
                        _ => {
                            if free_images {
                                self.kitty_images.clear();
                                self.kitty_order.clear();
                                self.kitty_bytes = 0;
                            }
                            self.virtual_placements.clear();
                            self.latest_placements.clear();
                            (true, None, None)
                        }
                    };
                ParseOutcome::Handled(ParsedImage {
                    image: None,
                    reply: Some(self.ack(&meta, true)),
                    advance_cursor: false,
                    clear_image: clear_all,
                    delete_id,
                    delete_image_id,
                })
            }
            _ => ParseOutcome::Consumed,
        }
    }

    /// Store a `U=1` virtual placement for later materialization.
    fn store_virtual(&mut self, meta: &KittyMeta) {
        self.virtual_placements.insert(
            meta.placement_id,
            VirtualPlacement {
                image_id: meta.id,
                cell_size: meta.cell_size,
                cell_offset: meta.cell_offset,
                z_index: meta.z_index,
            },
        );
        self.latest_placements.insert(meta.id, meta.placement_id);
    }

    /// Materialize a `U=1` virtual placement: build the full image,
    /// aspect-fitted to its `c × r` box, anchored at the placeholder cell.
    fn materialize(
        &mut self,
        image_id: u32,
        placement_id: u32,
        anchor: USizeVec2,
    ) -> Option<Image> {
        let (stored, base64) = self.kitty_images.get(&image_id)?.clone();
        let (w, h, rgba) = stored.decode(&base64)?;
        let vp = self.virtual_placements.get(&placement_id).or_else(|| {
            let latest = self.latest_placements.get(&image_id)?;
            self.virtual_placements.get(latest)
        })?;
        Some(Image {
            kind: ImageKind::Kitty,
            id: image_id,
            version: crate::parser::next_version(),
            anchor,
            cell_size: vp.cell_size,
            size: UVec2::new(w, h),
            cell_offset: vp.cell_offset,
            z_index: vp.z_index,
            aspect: UVec2::ONE,
            pixels: Arc::from(rgba),
        })
    }

    /// Crop a transmitted image to its source rect and position it at the
    /// cursor (full-text line = viewport top + cursor row).
    fn display(
        &self,
        meta: &KittyMeta,
        w: u32,
        h: u32,
        rgba: &[u8],
        ctx: ImageCtx,
    ) -> Image {
        let (cw, ch, cropped) = crop_rgba(
            rgba,
            w,
            h,
            meta.src.x,
            meta.src.y,
            meta.src_size.x,
            meta.src_size.y,
        );
        let col = ctx.cursor_col.saturating_sub(1);
        let viewport_row = ctx.cursor_row.saturating_sub(1);
        Image {
            kind: ImageKind::Kitty,
            id: if meta.placement_id != 0 {
                meta.placement_id
            } else {
                meta.id
            },
            version: crate::parser::next_version(),
            anchor: USizeVec2::new(col, ctx.screen_start + viewport_row),
            cell_size: meta.cell_size,
            size: UVec2::new(cw, ch),
            cell_offset: meta.cell_offset,
            z_index: meta.z_index,
            aspect: UVec2::ONE,
            pixels: Arc::from(cropped),
        }
    }

    /// Build the kitty acknowledgement (`Gi=<id>[,p=<pid>];OK|ENOENT`).
    /// `q` gates it: 0 = noisy, 1 = errors only, 2 = silent; id 0 never acks.
    fn ack(&self, meta: &KittyMeta, ok: bool) -> Vec<u8> {
        if meta.id == 0 || meta.quiet == 2 || (ok && meta.quiet == 1) {
            return Vec::new();
        }
        let mut reply = format!("\x1b_Gi={}", meta.id);
        if meta.placement_id != 0 {
            let _ = write!(reply, ",p={}", meta.placement_id);
        }
        if ok {
            reply.push_str(";OK\x1b\\");
        } else {
            reply.push_str(";ENOENT:no such image\x1b\\");
        }
        reply.into_bytes()
    }
}

/// Crop an RGBA raster to a source rectangle (`x`, `y`, `w`, `h`); `w=0` or
impl ImageParser for KittyParser {
    fn parse(
        &mut self,
        payload: &[u8],
        final_byte: u8,
        ctx: ImageCtx,
    ) -> ParseOutcome {
        // Modern form is `APC _G`; the legacy form shares `DCS q` with Sixel
        // and is distinguished by the `=` in its `key=value` header.
        if final_byte != b'G'
            && !(final_byte == b'q' && payload.contains(&b'='))
        {
            return ParseOutcome::Skip;
        }
        let Some((meta, data)) = parse_kitty_meta(payload) else {
            return ParseOutcome::Skip;
        };
        if meta.more {
            // `m=1`: append this chunk's base64; the start chunk's header is
            // authoritative. Cap the accumulated buffer so a stream of chunks
            // can't grow it without bound (each DCS payload is capped, but the
            // reassembly buffer spans payloads).
            match &mut self.kitty_pending {
                Some((_, base64))
                    if base64.len().saturating_add(data.len())
                        <= MAX_IMAGE_PAYLOAD =>
                {
                    base64.extend_from_slice(data);
                }
                Some(_) => {}
                None => self.kitty_pending = Some((meta, data.to_vec())),
            }
            return ParseOutcome::Consumed;
        }
        // `m=0`/absent: final (or only) chunk — reassemble + decode.
        let (meta, mut base64) =
            self.kitty_pending.take().unwrap_or((meta, Vec::new()));
        base64.extend_from_slice(data);
        self.place(meta, base64, ctx)
    }

    fn clear(&mut self) {
        self.kitty_images.clear();
        self.kitty_order.clear();
        self.kitty_bytes = 0;
        self.kitty_pending = None;
        self.virtual_placements.clear();
        self.latest_placements.clear();
    }

    fn materialize_placeholder(
        &mut self,
        image_id: u32,
        placement_id: u32,
        anchor: USizeVec2,
    ) -> Option<Image> {
        self.materialize(image_id, placement_id, anchor)
    }

    fn virtual_placement(
        &self,
        image_id: u32,
        placement_id: u32,
    ) -> Option<VirtualPlacement> {
        if let Some(vp) = self.virtual_placements.get(&placement_id) {
            return Some(*vp);
        }
        let latest = self.latest_placements.get(&image_id).copied()?;
        self.virtual_placements.get(&latest).copied().or_else(|| {
            self.virtual_placements
                .values()
                .find(|vp| vp.image_id == image_id)
                .copied()
        })
    }
}

/// `h=0` means "to the edge". Clamped to the raster.
fn crop_rgba(
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
#[path = "../tests/kitty_parser/mod.rs"]
mod parser_tests;
