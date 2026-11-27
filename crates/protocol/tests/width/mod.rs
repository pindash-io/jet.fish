// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

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
fn halfwidth_katakana_marks_are_single_cell() {
    assert_eq!(super::char_cell_width('\u{FF9E}'), 1);
    assert_eq!(super::char_cell_width('\u{FF9F}'), 1);
}

#[test]
fn hangul_jamo_vowels_and_finals_are_zero_width() {
    let ranges =
        [0x1160..=0x11FF_u32, 0xD7B0..=0xD7C6_u32, 0xD7CB..=0xD7FB_u32];
    for cp in ranges.into_iter().flatten() {
        let Some(c) = char::from_u32(cp) else { continue };
        assert_eq!(super::char_cell_width(c), 0, "U+{cp:04X} must be 0-width");
    }
}

#[test]
fn cell_col_byte_steps_over_merged_emoji() {
    // 中 is 2 cells, so cell 2 is the byte after it (3 bytes).
    assert_eq!(super::cell_col_byte("中ab", 0), 0);
    assert_eq!(super::cell_col_byte("中ab", 2), 3);
    // VS16 emoji merges to 2 cells: ⚠️ then 'x'; cell 2 is 'x' (byte 6).
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
