// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Decoded image (Sixel / kitty / iTerm2) in screen coordinates.

use std::sync::Arc;

/// A decoded image in screen coordinates. Lives in its own texture, not the
/// glyph atlas (cleared on font-size switches).
#[derive(Clone, Debug)]
pub struct ImageQuad {
    /// Top-left X, logical px.
    pub x: f32,
    /// Top-left Y, logical px.
    pub y: f32,
    /// Width, logical px.
    pub w: f32,
    /// Height, logical px.
    pub h: f32,
    /// Pixel width.
    pub width: u32,
    /// Pixel height.
    pub height: u32,
    /// Image id (kitty `i=`; Sixel/iTerm2 use 0).
    pub id: u32,
    /// Monotonic decode version, bumped per image content.
    pub version: u64,
    /// z-index; negative draws under the text.
    pub z_index: i32,
    /// RGBA pixels, `width × height × 4` bytes.
    pub pixels: Arc<[u8]>,
}
