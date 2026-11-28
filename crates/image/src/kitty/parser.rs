// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Kitty image storage, placement, and the `m=` chunk-reassembly buffer.

use std::collections::VecDeque;
use std::fmt::Write as _;
use std::sync::Arc;

use glam::{USizeVec2, UVec2};
use hashbrown::HashMap;

use super::decode::crop_rgba;
use super::meta::parse_kitty_meta;
use super::meta::{KittyImage, KittyMeta};
use crate::MAX_IMAGE_PAYLOAD;
use crate::parser::{
    Image, ImageCtx, ImageKind, ImageParser, ParseOutcome, ParsedImage,
    VirtualPlacement,
};

/// Kitty graphics (`DCS q` legacy / `APC _G` modern): owns the stored-image map
/// and the `m=` chunk-reassembly buffer.
#[derive(Default)]
pub struct KittyParser {
    images: HashMap<u32, (KittyMeta, Vec<u8>)>,
    order: VecDeque<u32>,
    stored_bytes: usize,
    pending: Option<(KittyMeta, Vec<u8>)>,
    /// `a=p,U=1` virtual placements, keyed by placement id (`p=`).
    virtual_placements: HashMap<u32, VirtualPlacement>,
    /// The most recent placement id per image id.
    latest_placements: HashMap<u32, u32>,
}

impl KittyParser {
    /// Insert into the upload cache (still base64), evicting FIFO past
    /// `MAX_IMAGE_PAYLOAD × 4`; decode waits for placement.
    fn cache_insert(&mut self, meta: KittyMeta, base64: Vec<u8>) {
        let id = meta.id;
        let size = base64.len();
        if let Some((_, old)) = self.images.insert(id, (meta, base64)) {
            self.stored_bytes = self.stored_bytes.saturating_sub(old.len());
        } else {
            self.order.push_back(id);
        }
        self.stored_bytes = self.stored_bytes.saturating_add(size);
        const CAP: usize = MAX_IMAGE_PAYLOAD * 4;
        while self.stored_bytes > CAP {
            let Some(id) = self.order.pop_front() else {
                break;
            };
            if let Some((_, base64)) = self.images.remove(&id) {
                self.stored_bytes =
                    self.stored_bytes.saturating_sub(base64.len());
            }
        }
    }

    /// A handled outcome carrying only an acknowledgement reply.
    fn ack_handled(&self, meta: &KittyMeta, ok: bool) -> ParseOutcome {
        ParseOutcome::Handled(ParsedImage {
            reply: Some(self.ack(meta, ok)),
            ..ParsedImage::default()
        })
    }

    /// A handled outcome placing one image (cursor advance is the kitty
    /// `C=0` default, gated on a declared cell size).
    fn image_handled(&self, meta: &KittyMeta, image: Image) -> ParseOutcome {
        ParseOutcome::Handled(ParsedImage {
            image: Some(image),
            reply: Some(self.ack(meta, true)),
            advance_cursor: meta.cursor_move
                && (meta.cell_size.x > 0 || meta.cell_size.y > 0),
            ..ParsedImage::default()
        })
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
            // Decode is deferred to `p`/`P`/`materialize`.
            't' => {
                self.cache_insert(meta.clone(), base64);
                self.ack_handled(&meta, true)
            }
            // Transmit + display at the cursor.
            'T' => {
                let Some((w, h, rgba)) = meta.decode(&base64) else {
                    return self.ack_handled(&meta, false);
                };
                self.cache_insert(meta.clone(), base64);
                if meta.unicode_placeholder {
                    self.store_virtual(&meta);
                    return self.ack_handled(&meta, true);
                }
                let image = display(&meta, w, h, &rgba, ctx);
                self.image_handled(&meta, image)
            }
            // Place a previously transmitted image at the cursor.
            'p' | 'P' => {
                if meta.unicode_placeholder {
                    self.store_virtual(&meta);
                    return self.ack_handled(&meta, true);
                }
                match self.images.get(&meta.id).cloned() {
                    Some((stored, base64)) => {
                        let Some((w, h, rgba)) = stored.decode(&base64) else {
                            return self.ack_handled(&meta, false);
                        };
                        let image = display(&meta, w, h, &rgba, ctx);
                        self.image_handled(&meta, image)
                    }
                    None => self.ack_handled(&meta, false),
                }
            }
            // Capability query: reply OK for a supported medium + format.
            'q' => {
                let ok = matches!(meta.medium, 'd' | 'f' | 't')
                    && matches!(meta.format, 24 | 32 | 100);
                self.ack_handled(&meta, ok)
            }
            // Delete (`a=d`): lowercase removes placements only, uppercase also frees
            // the image data.
            'd' => {
                let free_images = meta.delete_spec.is_ascii_uppercase();
                let (clear_all, delete_id, delete_image_id) =
                    match meta.delete_spec {
                        'i' | 'I' => {
                            if free_images {
                                if let Some((_, base64)) =
                                    self.images.remove(&meta.id)
                                {
                                    self.stored_bytes = self
                                        .stored_bytes
                                        .saturating_sub(base64.len());
                                }
                                self.order.retain(|&id| id != meta.id);
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
                                self.images.clear();
                                self.order.clear();
                                self.stored_bytes = 0;
                            }
                            self.virtual_placements.clear();
                            self.latest_placements.clear();
                            (true, None, None)
                        }
                    };
                ParseOutcome::Handled(ParsedImage {
                    reply: Some(self.ack(&meta, true)),
                    clear_image: clear_all,
                    delete_id,
                    delete_image_id,
                    ..ParsedImage::default()
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
        let (stored, base64) = self.images.get(&image_id)?.clone();
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
            // `m=1`: append the chunk's base64; the start chunk's header is
            // authoritative, and the buffer is capped across payloads.
            match &mut self.pending {
                Some((_, base64))
                    if base64.len().saturating_add(data.len())
                        <= MAX_IMAGE_PAYLOAD =>
                {
                    base64.extend_from_slice(data);
                }
                Some(_) => {}
                None => self.pending = Some((meta, data.to_vec())),
            }
            return ParseOutcome::Consumed;
        }
        // `m=0`/absent: the final (or only) chunk, so reassemble and decode.
        let (meta, mut base64) =
            self.pending.take().unwrap_or((meta, Vec::new()));
        base64.extend_from_slice(data);
        self.place(meta, base64, ctx)
    }

    fn clear(&mut self) {
        self.images.clear();
        self.order.clear();
        self.stored_bytes = 0;
        self.pending = None;
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
/// Place a decoded kitty image at `ctx`'s cursor: crop the source rectangle,
/// then position it in the grid. `None` when the payload does not decode.
#[must_use]
pub fn place_kitty(img: &KittyImage, ctx: ImageCtx) -> Option<Image> {
    let (w, h, rgba) = img.meta.decode(&img.pixels)?;
    Some(display(&img.meta, w, h, &rgba, ctx))
}

/// Build the placed [`Image`] for a decoded payload.
fn display(
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
        id: if meta.placement_id != 0 { meta.placement_id } else { meta.id },
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
