// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Rendering facet: glyph layout, rasterization state, cell metrics, blink.

use std::sync::Arc;

use glam::Vec2;
use vello_cpu::Pixmap;

use jetfish_protocol::Result;
use jetfish_text::Primitive;

use crate::ImageQuad;
use crate::engine::selection::CellPoint;

/// A resolved glyph batch: the positioned primitives plus the pixmaps.
pub type GlyphBatch = (Arc<Vec<Primitive>>, Vec<Arc<Pixmap>>);

/// Glyph layout, rasterization state, cell metrics, and blink.
pub trait Rendering {
    /// Lay out and rasterize the visible glyphs.
    fn layout(
        &mut self,
        width: f32,
        viewport_top: f32,
        viewport_bottom: f32,
    ) -> Result<GlyphBatch>;

    /// Set the active text selection ([`CellPoint`]s) so the layout can
    /// break multi-cell ligatures at the selection edges. `rectangle` marks an
    /// Alt+drag column-wise selection. Either endpoint `None` clears it.
    fn set_selection(
        &mut self,
        _anchor: Option<CellPoint>,
        _active: Option<CellPoint>,
        _rectangle: bool,
    ) {
    }

    /// Set the hyperlink to underline while the Option key is held (the hover
    /// reveal). `uri=None` clears it. Returns true when the hovered link
    /// changed (the caller should queue a redraw).
    fn set_hover_link(
        &mut self,
        _uri: Option<&str>,
        _abs_line: usize,
        _col: usize,
    ) -> bool {
        false
    }

    /// Images to render this frame, or `None` when empty.
    fn images(&self) -> Option<Vec<ImageQuad>> {
        None
    }

    /// Set the base font size; invalidates the layout.
    fn set_font_size(&mut self, size: f32);

    /// Set the physical-to-logical pixel scale (window scale factor);
    /// invalidates the layout.
    fn set_scale_factor(&mut self, _scale: f32) {}

    /// The base font size in logical pixels.
    fn font_size(&self) -> f32;

    /// Bumps when glyphs rasterize into the atlas.
    fn atlas_generation(&self) -> u64 {
        0
    }

    /// One cell in physical pixels.
    fn cell_size(&self) -> Vec2 {
        Vec2::ZERO
    }

    /// Viewport rows.
    fn rows(&self) -> usize {
        0
    }

    /// Whether the last layout enqueued missing glyphs.
    fn pending_raster(&self) -> bool {
        false
    }

    /// Whether layout returns viewport-relative glyphs.
    fn viewport_cropped(&self) -> bool {
        false
    }

    /// Flip the text-blink phase; returns true to redraw.
    fn toggle_blink(&mut self) -> bool {
        false
    }

    /// Mark the cursor-blink layout dirty after the host flipped the shared
    /// phase; returns true to redraw.
    fn toggle_cursor_blink(&mut self) -> bool {
        false
    }

    /// Whether any span blinks.
    fn has_blink(&self) -> bool {
        false
    }

    /// Whether the cursor blinks.
    fn cursor_blink(&self) -> bool {
        false
    }
}
