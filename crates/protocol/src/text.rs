// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Pure byte-offset text helpers (char boundaries, line splitting); cell
//! widths live in [`crate::width`].

use core::range::Range;

use crate::{cell_col_byte, line_cell_width};

/// Double-click word-selection boundary characters: a word is a maximal run
/// without any of them, and a click on one selects that single cell.
pub const WORD_BOUNDARIES: &str = "\0 \t'\"│`|:;,()[]{}<>$";

/// Double-click word bounds for column `col` (inclusive), using
/// [`WORD_BOUNDARIES`].
#[must_use]
pub fn semantic_word_bounds(line: &str, col: usize) -> (usize, usize) {
    let byte = cell_col_byte(line, col);
    let mut left = 0_usize;
    for (i, c) in line.char_indices() {
        if i >= byte {
            break;
        }
        if WORD_BOUNDARIES.contains(c) {
            left = i + c.len_utf8();
        }
    }
    let mut right = line.len();
    for (i, c) in line.char_indices() {
        if i >= byte && WORD_BOUNDARIES.contains(c) {
            right = i;
            break;
        }
    }
    let left_col = line_cell_width(&line[..left]);
    let right_excl = line_cell_width(&line[..right]);
    let right_col =
        if right_excl > left_col { right_excl - 1 } else { left_col };
    (left_col, right_col)
}

/// Byte ranges of every line (split on `\n`).
#[must_use]
pub fn text_line_ranges(text: &str) -> Vec<Range<usize>> {
    let newlines = memchr::memchr_iter(b'\n', text.as_bytes()).count();
    let mut ranges = Vec::with_capacity(newlines + 1);
    let mut start = 0;
    for i in memchr::memchr_iter(b'\n', text.as_bytes()) {
        ranges.push(Range { start, end: i });
        start = i + 1;
    }
    ranges.push(Range { start, end: text.len() });
    ranges
}

/// Byte offset of the start of the `line_index`-th line (0-based).
#[must_use]
#[inline]
pub fn line_start_byte(text: &str, line_index: usize) -> usize {
    if line_index == 0 {
        return 0;
    }
    memchr::memchr_iter(b'\n', text.as_bytes())
        .nth(line_index - 1)
        .map_or(text.len(), |i| i + 1)
}
