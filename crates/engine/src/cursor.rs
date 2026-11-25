// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Cursor rendering helpers shared by the terminal backends.

use crate::engine::{Primitive, PrimitiveKind};

/// Invert the text glyphs in the cursor cell; solid rects and color/bitmap
/// glyphs (emoji) are skipped — tinting a color glyph flashes it to the
/// cursor's background. `text_color` recolors, `behind_color` becomes
/// `bg_color` for the gamma-correct blend, `block_w` spans a wide cursor.
/// Columns use `round` (not `floor`): a snapped x off the cell boundary
/// would floor into the previous cell.
pub fn invert_cursor_cell(
    glyphs: &mut [Primitive],
    row: usize,
    col: usize,
    cell_w: f32,
    cell_h: f32,
    block_w: f32,
    text_color: [f32; 4],
    behind_color: [f32; 4],
) {
    // The cursor block spans `cells` columns (1, or 2 over a wide glyph).
    let cells = (block_w / cell_w.max(1.0)).round().max(1.0) as isize;
    let (row, col) = (row as isize, col as isize);
    for g in glyphs {
        if g.kind != PrimitiveKind::Mask {
            continue;
        }
        // Row is the glyph's vertical center, not its ink top: box-drawing
        // glyphs overhang the row top, so an ink-top floor would invert the
        // row below the cursor.
        let gr = ((g.pos.y + g.size.y * 0.5) / cell_h).floor() as isize;
        // The PEN (`pos.x - bearing_x`), not the ink origin: a right-column
        // braille dot has a large left bearing whose ink origin
        // `round`-spills into the next cell.
        let gc = ((g.pos.x - g.bearing_x) / cell_w).round() as isize;
        if gr == row && gc >= col && gc < col + cells {
            g.color = text_color;
            g.bg_color = behind_color;
            // The glyph keeps its natural position; re-centering shifted
            // normal glyphs by their bearing asymmetry.
        }
    }
}

#[cfg(test)]
#[path = "../tests/cursor/mod.rs"]
mod tests;
