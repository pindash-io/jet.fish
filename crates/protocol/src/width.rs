// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Byte↔cell width helpers.

// Re-exported from `vtty_width` — the single width-model source of truth:
// the byte↔cell walkers and the write-order Layer-3 model, so the line and
// cell screens share one implementation.
pub use vtty_width::{
    EmojiTrace, SplitBoundaries, cell_col_byte_ceil_from_widths,
    cell_col_byte_from_widths, cell_width, cell_width_sum, cell_widths,
    classify_cell, grapheme_end_from_widths, grapheme_tail_end_from_widths,
    splice_offset, split_widths, wrap_width,
};

/// The byte offset just past the trailing width-0 marks at `byte`. Widths
/// come from the whole `text` so a ZWJ continuation stays 0-width.
#[must_use]
#[inline]
pub fn grapheme_tail_end(text: &str, byte: usize) -> usize {
    grapheme_tail_end_from_widths(text, &cell_widths(text), byte)
}

// The walkers take per-char widths so both width sources (whole-text and
// split-DSA) share one implementation.

/// Display cell width of a char (wide/CJK = 2, else 1), 0 for control chars.
/// Delegates to [`crate::cell_width`].
#[must_use]
#[inline]
pub fn char_cell_width(c: char) -> usize {
    cell_width(c).unwrap_or(0)
}

/// Display cell width of a line.
#[must_use]
#[inline]
pub fn line_cell_width(line: &str) -> usize {
    // `cell_width_sum` is a single allocation-free pass with its own ASCII
    // fast path (one cell per byte).
    cell_width_sum(line)
}

/// Byte offset of a cell column, clamped to the line end (floor semantics:
/// a wide char's second cell maps to its start).
#[must_use]
#[inline]
pub fn cell_col_byte(line: &str, cell_col: usize) -> usize {
    vtty_width::cell_col_byte_widths(line, cell_col)
}

/// The char covering cell column `col`; a wide char's continuation cell
/// returns `' '`, `None` past the row.
#[must_use]
#[inline]
pub fn row_cell_char(text: &str, col: usize) -> Option<char> {
    let mut cell = 0_usize;
    for c in text.chars() {
        let w = char_cell_width(c).max(1);
        if col < cell + w {
            return if col == cell { Some(c) } else { Some(' ') };
        }
        cell += w;
    }
    None
}

/// DECRQCRA (`CSI Pi;Pg;Pt;Pl;Pb;Pr * y`) rectangle checksum.
///
/// `row_text` returns the logical text of a 0-based viewport row (`None` past
/// the screen's content). The rectangle is 0-based inclusive. Sums the full
/// Unicode codepoint of every cell, counting blank/undrawn/past-end cells as
/// a space (32), and masks to 16 bits. No negation and no attribute bits —
/// the simple xterm `checksumExtension` calculation esctest exercises with
/// `--xterm-checksum` (`no-negate,no-attributes,no-trim,full`).
#[must_use]
#[inline]
pub fn decrcra_checksum(
    mut row_text: impl FnMut(usize) -> Option<String>,
    top: usize,
    left: usize,
    bottom: usize,
    right: usize,
) -> u32 {
    let mut sum: u32 = 0;
    for row in top..=bottom {
        let text = row_text(row).unwrap_or_default();
        for col in left..=right {
            let ch = row_cell_char(&text, col).unwrap_or(' ');
            sum = sum.wrapping_add(ch as u32);
        }
    }
    sum & 0xffff
}

#[cfg(test)]
#[path = "../tests/width/mod.rs"]
mod tests;
