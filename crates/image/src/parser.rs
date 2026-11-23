// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Pluggable DCS/APC image-protocol parsers (Sixel, kitty, custom). Each
//! payload is offered to each parser in order; the first recognizer wins.
//! Concrete parsers live in [`crate::sixel`] / [`crate::kitty`].

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use glam::{USizeVec2, UVec2, Vec2};

/// Monotonic stamp for every decoded [`Image`]: the host's cheap change
/// detector (same id + same version ⇒ same pixels).
static IMAGE_VERSION: AtomicU64 = AtomicU64::new(0);

/// The next decoded-image version.
#[must_use]
#[inline]
pub(crate) fn next_version() -> u64 {
    IMAGE_VERSION.fetch_add(1, Ordering::Relaxed)
}

/// A DCS/APC image-protocol parser (Sixel, kitty graphics, or custom).
pub trait ImageParser: 'static {
    /// Try to decode a buffered payload. `final_byte` is `q` (`DCS q` — Sixel
    /// or legacy kitty) or `G` (`APC _G` — modern kitty).
    fn parse(
        &mut self,
        payload: &[u8],
        final_byte: u8,
        ctx: ImageCtx,
    ) -> ParseOutcome;

    /// Drop protocol state (stored images, chunk buffer) on `ED` / full reset.
    fn clear(&mut self) {}

    /// Materialize a `U=1` virtual placement for a `U+10EEEE` placeholder
    /// cell. `image_id`/`placement_id` are decoded from the cell's fg and
    /// underline colors; `anchor` is the placeholder cell's grid position.
    /// Returns `None` when the ids are unknown (not this parser).
    fn materialize_placeholder(
        &mut self,
        _image_id: u32,
        _placement_id: u32,
        _anchor: USizeVec2,
    ) -> Option<Image> {
        None
    }

    /// The matching `U=1` virtual placement (geometry only, no pixels), used
    /// to re-anchor and re-scale an already-placed image in place when a
    /// resize re-sends `a=p` with a new `c`/`r` box. Returns `None` when the
    /// ids are unknown.
    fn virtual_placement(
        &self,
        _image_id: u32,
        _placement_id: u32,
    ) -> Option<VirtualPlacement> {
        None
    }
}

/// A decoded terminal image (Sixel / kitty / iTerm2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    /// Which protocol produced the image — the host scales Sixel pixels at
    /// 6-per-cell but renders kitty/iTerm2 pixels 1:1 in device pixels.
    pub kind: ImageKind,
    /// Kitty image id (`i=`); Sixel/iTerm2 use 0.
    pub id: u32,
    /// Monotonic version, bumped per decode.
    pub version: u64,
    /// Anchor position: `.x` = 0-based cell column, `.y` = full-text line
    /// (0-based). The viewport row is computed at render time
    /// (`anchor.y - screen_start()`) so the image scrolls with its text line
    /// and stays put across resizes.
    pub anchor: USizeVec2,
    /// Display size in cells (`c=`/`r=`) — 0 means the pixel size.
    pub cell_size: UVec2,
    /// Size in pixels.
    pub size: UVec2,
    /// Pixel offset within the starting cell (`X=`/`Y=`, kitty).
    pub cell_offset: UVec2,
    /// z-index (`z=`) — negative draws the image under the text.
    pub z_index: i32,
    /// Pixel aspect ratio (`Pan:Pad`) — Sixel default 2:1, kitty 1:1.
    pub aspect: UVec2,
    /// RGBA pixels (`width × height × 4` bytes). Sixel is opaque (α=255);
    /// kitty/iTerm2 carry their source alpha for transparent backgrounds.
    pub pixels: Arc<[u8]>,
}

/// The image protocol that produced an [`Image`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageKind {
    /// DEC Sixel — pixels are 6-per-cell vertically, aspect-corrected.
    Sixel,
    /// Kitty graphics — pixels are 1:1 device pixels.
    Kitty,
}

/// A kitty `a=p,U=1` virtual placement: the image materializes later, when a
/// `U+10EEEE` placeholder cell carrying the matching ids is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VirtualPlacement {
    /// Kitty image id (`i=`) of the transmitted image to draw.
    pub image_id: u32,
    /// Display box in cells (`c=`/`r=`) — the image is aspect-fitted into it.
    pub cell_size: UVec2,
    /// Pixel offset within the starting cell (`X=`/`Y=`).
    pub cell_offset: UVec2,
    /// z-index (`z=`) — negative draws under the text.
    pub z_index: i32,
}

impl Image {
    /// The physical-pixel scale per source pixel. Sixel pixels are 1:1
    /// device pixels, aspect-corrected (`Pan:Pad`, default 2:1); kitty/
    /// iTerm2 pixels are 1:1 device pixels.
    #[inline]
    #[must_use]
    pub fn pixel_scale(&self) -> Vec2 {
        match self.kind {
            ImageKind::Sixel => {
                Vec2::ONE.with_x(self.aspect.x as f32 / self.aspect.y as f32)
            }
            ImageKind::Kitty => Vec2::ONE,
        }
    }

    /// The display size in physical pixels, following kitty's `a=p`
    /// semantics: both `c` and `r` set → the image is scaled to FIT WITHIN
    /// the `c×r` box, preserving aspect (letterboxed/pillarboxed); only one
    /// set → that axis is exact and the other follows the source aspect;
    /// neither set → native pixel size.
    #[inline]
    #[must_use]
    pub fn display_size(&self, cell: Vec2) -> Vec2 {
        let natural = self.size.as_vec2() * self.pixel_scale();
        // The per-axis scale to the `c×r` box (guarding the division for a
        // zero-sized source). An unset axis is unbounded — its ratio is
        // ignored below.
        let ratio =
            self.cell_size.as_vec2() * cell / natural.max(Vec2::splat(1.0));
        let scale = match (self.cell_size.x > 0, self.cell_size.y > 0) {
            (true, true) => ratio.min_element(),
            (true, false) => ratio.x,
            (false, true) => ratio.y,
            (false, false) => 1.0,
        };
        natural * scale
    }

    /// The display size in cells, rounding each axis up to the next whole
    /// cell. Used for viewport culling. When both `c` and `r` are set the
    /// image occupies that whole box (the aspect-fit may leave empty margin
    /// inside it), so the culling box is `c × r` itself.
    #[inline]
    #[must_use]
    pub fn cell_extent(&self, cell: Vec2) -> UVec2 {
        if self.cell_size.x > 0 && self.cell_size.y > 0 {
            return self.cell_size;
        }
        let px = self.display_size(cell);
        (px / cell.max(Vec2::splat(1.0))).ceil().as_uvec2()
    }

    /// Whether the image's cell extent overlaps the viewport. `full_line` is
    /// the absolute line (`anchor.y` + scrollback offset); `view_start` is
    /// the viewport-top line; `rows`/`cols` are the viewport grid; `cell` is
    /// the physical cell size. A partial overlap still renders — the host
    /// clips the remainder, keeping the image glued to its anchor while
    /// scrolling.
    #[inline]
    pub fn intersects_viewport(
        &self,
        full_line: isize,
        view_start: usize,
        rows: usize,
        cols: usize,
        cell: Vec2,
    ) -> bool {
        let extent = self.cell_extent(cell);
        let row = full_line - view_start as isize;
        let col = self.anchor.x as isize;
        let visible_v = row + extent.y as isize > 0 && row < rows as isize;
        let visible_h = col + extent.x as isize > 0 && col < cols as isize;
        visible_v && visible_h
    }
}

/// A snapshot of the screen state an image parser needs for placement.
#[derive(Clone, Copy, Debug)]
pub struct ImageCtx {
    /// 1-based cursor column.
    pub cursor_col: usize,
    /// 1-based cursor row (viewport-relative).
    pub cursor_row: usize,
    /// Full-text line (0-based) the viewport top is anchored to.
    pub screen_start: usize,
}

/// Side effects a parser asks the terminal to apply.
#[derive(Debug, Default)]
pub struct ParsedImage {
    /// The image to place (cropped + positioned), if any.
    pub image: Option<Image>,
    /// A protocol reply to write to the PTY (e.g. a kitty acknowledgement).
    pub reply: Option<Vec<u8>>,
    /// Advance the cursor past the placed image (kitty `C=0` default).
    pub advance_cursor: bool,
    /// Clear every displayed image (kitty `a=d` with `d=a`/`d=A`).
    pub clear_image: bool,
    /// Delete only the image with this id (kitty `a=d` with `d=i`/`d=I`).
    pub delete_id: Option<u32>,
    /// Delete the stored image data with this id (kitty `a=d` with `d=I`,
    /// freeing the upload cache so a later re-place fails).
    pub delete_image_id: Option<u32>,
}

/// What a parser did with a payload.
#[derive(Debug)]
pub enum ParseOutcome {
    /// Not this protocol — try the next parser.
    Skip,
    /// Recognized with no side effects (e.g. a buffered `m=1` chunk).
    Consumed,
    /// Recognized: apply [`ParsedImage`].
    Handled(ParsedImage),
}

/// The ordered registry of image parsers for a terminal.
#[derive(Default)]
pub struct ImageParsers {
    parsers: Vec<Box<dyn ImageParser>>,
}

impl std::fmt::Debug for ImageParsers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImageParsers")
            .field("len", &self.parsers.len())
            .finish()
    }
}

impl ImageParsers {
    /// The built-in parsers: kitty graphics + DEC Sixel.
    #[must_use]
    pub fn default_parsers() -> Self {
        let mut parsers = Self::default();
        #[cfg(feature = "kitty")]
        parsers.push(crate::KittyParser::default());
        #[cfg(feature = "sixel")]
        parsers.push(crate::SixelParser);
        parsers
    }

    /// Register a custom parser (appended after the built-ins).
    #[inline]
    pub fn push(&mut self, parser: impl ImageParser) {
        self.parsers.push(Box::new(parser));
    }

    /// Iterate the parsers mutably.
    #[inline]
    pub fn iter_mut(
        &mut self,
    ) -> impl Iterator<Item = &mut Box<dyn ImageParser>> {
        self.parsers.iter_mut()
    }

    /// Clear every parser's state.
    #[inline]
    pub fn clear(&mut self) {
        for parser in &mut self.parsers {
            parser.clear();
        }
    }

    /// Materialize a `U=1` virtual placement across all parsers; the first
    /// parser that recognizes the ids wins.
    #[inline]
    #[must_use]
    pub fn materialize_placeholder(
        &mut self,
        image_id: u32,
        placement_id: u32,
        anchor: USizeVec2,
    ) -> Option<Image> {
        for parser in &mut self.parsers {
            if let Some(image) =
                parser.materialize_placeholder(image_id, placement_id, anchor)
            {
                return Some(image);
            }
        }
        None
    }

    /// The matching `U=1` virtual placement (geometry only), for re-anchoring
    /// and re-scaling an already-placed image in place.
    #[inline]
    #[must_use]
    pub fn virtual_placement(
        &self,
        image_id: u32,
        placement_id: u32,
    ) -> Option<VirtualPlacement> {
        for parser in &self.parsers {
            if let Some(vp) = parser.virtual_placement(image_id, placement_id)
            {
                return Some(vp);
            }
        }
        None
    }
}

#[cfg(test)]
#[path = "../tests/parser/mod.rs"]
mod tests;
