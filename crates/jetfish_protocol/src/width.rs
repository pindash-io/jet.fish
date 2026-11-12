// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Byte↔cell width helpers shared by the line screen and its spans.

/// The byte offset just past the trailing width-0 marks at `byte`. Widths
/// come from the whole `text` so a ZWJ continuation stays 0-width.
#[inline]
pub fn grapheme_tail_end(text: &str, byte: usize) -> usize {
    grapheme_tail_end_from_widths(text, &crate::cell_widths(text), byte)
}

// The walkers take per-char widths so both width sources (whole-text and
// split-DSA) share one implementation.

// Re-exported from `vtty_width` — the single width-model source of truth.
pub use vtty_width::{
    cell_col_byte_ceil_from_widths, cell_col_byte_from_widths,
    grapheme_end_from_widths, grapheme_tail_end_from_widths,
};

/// Display cell width of a char (wide/CJK = 2, else 1), 0 for control chars.
/// Delegates to [`crate::cell_width`].
#[inline]
pub fn char_cell_width(c: char) -> usize {
    crate::cell_width(c).unwrap_or(0)
}

/// Display cell width of a line.
#[inline]
pub fn line_cell_width(line: &str) -> usize {
    // `cell_width_sum` is a single allocation-free pass with its own ASCII
    // fast path (one cell per byte).
    crate::cell_width_sum(line)
}

/// Byte offset of a cell column, clamped to the line end (floor semantics:
/// a wide char's second cell maps to its start).
#[inline]
pub fn cell_col_byte(line: &str, cell_col: usize) -> usize {
    vtty_width::cell_col_byte_widths(line, cell_col)
}

/// The char covering cell column `col`; a wide char's continuation cell
/// returns `' '`, `None` past the row.
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

// The write-order Layer-3 model lives in `vtty_width::write_order` (the
// single width-model source of truth); re-exported so the line and cell
// screens share ONE implementation.
pub use vtty_width::{
    EmojiTrace, SplitBoundaries, classify_cell, splice_offset, split_widths,
    wrap_width,
};

#[cfg(test)]
mod tests {
    use super::{decrcra_checksum, row_cell_char};

    #[test]
    fn row_cell_char_maps_ascii_and_wide() {
        // ASCII: column == char index.
        assert_eq!(row_cell_char("AB", 0), Some('A'));
        assert_eq!(row_cell_char("AB", 1), Some('B'));
        assert_eq!(row_cell_char("AB", 2), None);
        // A wide CJK char occupies two cells; its continuation is a space.
        let wide = "\u{4e2d}"; // 中 (width 2)
        assert_eq!(row_cell_char(wide, 0), Some('中'));
        assert_eq!(row_cell_char(wide, 1), Some(' '));
        assert_eq!(row_cell_char(wide, 2), None);
    }

    #[test]
    fn decrcra_checksum_sums_codepoints_and_blanks() {
        // 2×2 area of A/B/C/D: 0x41+0x42+0x43+0x44 = 0x10A.
        let rows = |row: usize| match row {
            0 => Some("AB".to_string()),
            1 => Some("CD".to_string()),
            _ => None,
        };
        assert_eq!(decrcra_checksum(rows, 0, 0, 1, 1), 0x010A);

        // A 1×1 area past the content counts one blank cell (space = 0x20).
        let empty = |_row: usize| -> Option<String> { None };
        assert_eq!(decrcra_checksum(empty, 0, 0, 0, 0), 0x0020);

        // Columns past the row end count as spaces.
        let short = |row: usize| match row {
            0 => Some("A".to_string()),
            _ => None,
        };
        assert_eq!(decrcra_checksum(short, 0, 0, 0, 1), 0x41 + 0x20);
    }

    #[test]
    fn cell_col_byte_steps_over_merged_emoji() {
        // 中 is 2 cells, so cell 2 is the byte after it (3 bytes).
        assert_eq!(super::cell_col_byte("中ab", 0), 0);
        assert_eq!(super::cell_col_byte("中ab", 2), 3);
        // VS16 emoji merges to 2 cells: ⚠️ then 'x' — cell 2 is 'x' (byte 6).
        assert_eq!(super::cell_col_byte("⚠\u{FE0F}x", 2), 6);
        // ZWJ family merges to 2 cells; cell 2 is 'x'.
        assert_eq!(
            super::cell_col_byte("👨\u{200D}👩\u{200D}👧\u{200D}👦x", 2),
            "👨\u{200D}👩\u{200D}👧\u{200D}👦".len(),
        );
        // A column landing in the SECOND cell of a wide char maps back to
        // that wide char (not to the char after it).
        assert_eq!(super::cell_col_byte("中ab", 1), 0);
        assert_eq!(super::cell_col_byte("a中b", 2), 1);
        assert_eq!(super::cell_col_byte("⚠\u{FE0F}x", 1), 0);
    }
}
