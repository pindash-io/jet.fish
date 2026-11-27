// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

use glam::{U16Vec2, Vec2};

use super::*;

fn mark() -> Primitive {
    Primitive {
        pos: Vec2::new(73.0, 4.0),
        size: Vec2::new(7.0, 6.0),
        bearing_x: 0.0,
        atlas_pos: U16Vec2::new(1, 26),
        atlas_size: U16Vec2::new(7, 6),
        color: [1.0, 0.0, 0.0, 1.0],
        bg_color: [0.0; 4],
        kind: PrimitiveKind::Mask,
        page_index: 0,
    }
}

/// Cursor in: the mark keeps its natural position and is only recolored.
#[test]
fn cursor_in_keeps_mark_natural_y() {
    let mut glyphs = [mark()];
    invert_cursor_cell(
        &mut glyphs,
        0,
        4,
        17.0,
        34.0,
        17.0,
        [1.0; 4],
        [0.0; 4],
    );
    assert_eq!(glyphs[0].pos.y, 4.0, "mark must not be vertically centered");
    assert_eq!(
        glyphs[0].pos.x, 73.0,
        "mark keeps its natural x (no re-centering)"
    );
    assert_eq!(
        glyphs[0].color, [1.0; 4],
        "mark is recolored to the inverted text color"
    );
    assert_eq!(glyphs[0].bg_color, [0.0; 4], "mark bg is behind color");
}

/// Cursor moves OUT of the accent's cell: the glyph is untouched.
#[test]
fn cursor_out_leaves_mark_untouched() {
    let mut glyphs = [mark()];
    invert_cursor_cell(
        &mut glyphs,
        0,
        5,
        17.0,
        34.0,
        17.0,
        [1.0; 4],
        [0.0; 4],
    );
    assert_eq!(glyphs[0].pos.x, 73.0);
    assert_eq!(glyphs[0].pos.y, 4.0);
    assert_eq!(glyphs[0].color, [1.0, 0.0, 0.0, 1.0]);
}

/// A row-1 glyph whose ink overhangs the row top must not be classified
/// as row 0 (the nvim indent-guide `│` vanishing under the cursor).
#[test]
fn row_below_overhang_is_not_cursor_row() {
    let mut glyphs = [Primitive {
        pos: Vec2::new(58.0, 33.0),
        size: Vec2::new(4.0, 37.0),
        ..mark()
    }];
    invert_cursor_cell(
        &mut glyphs,
        0,
        3,
        17.0,
        34.0,
        17.0,
        [1.0; 4],
        [0.0; 4],
    );
    assert_eq!(glyphs[0].pos.x, 58.0, "row-1 glyph must not re-center");
    assert_eq!(glyphs[0].pos.y, 33.0);
    assert_eq!(glyphs[0].color, [1.0, 0.0, 0.0, 1.0]);
}

/// A normal glyph keeps its natural x; re-centering shifted it by its
/// bearing asymmetry.
#[test]
fn cursor_does_not_recenter_normal_glyph() {
    let mut glyphs = [Primitive {
        pos: Vec2::new(19.0, 5.0),
        size: Vec2::new(14.0, 21.0),
        ..mark()
    }];
    invert_cursor_cell(
        &mut glyphs,
        0,
        1,
        17.0,
        34.0,
        17.0,
        [1.0; 4],
        [0.0; 4],
    );
    assert_eq!(
        glyphs[0].pos.x, 19.0,
        "normal glyph must keep its natural x (no 0.5px jitter)"
    );
    assert_eq!(glyphs[0].color, [1.0; 4]);
}

/// A 2-cell cursor inverts both columns of a wide glyph (a ZWJ family
/// emoji's two members at adjacent cells).
#[test]
fn wide_cursor_inverts_both_columns() {
    let mut glyphs = [
        Primitive {
            pos: Vec2::new(17.4, 9.0),
            size: Vec2::new(16.1, 16.1),
            ..mark()
        },
        Primitive {
            pos: Vec2::new(34.4, 9.0),
            size: Vec2::new(16.1, 16.1),
            ..mark()
        },
        // A neighbour at column 3 must NOT be inverted.
        Primitive {
            pos: Vec2::new(51.0, 9.0),
            size: Vec2::new(14.0, 21.0),
            ..mark()
        },
    ];
    // Cursor snapped to the family's LEADING cell (col 1), spanning 2
    // cells (block_w = 2 × 17).
    invert_cursor_cell(
        &mut glyphs,
        0,
        1,
        17.0,
        34.0,
        34.0,
        [1.0; 4],
        [0.0; 4],
    );
    assert_eq!(glyphs[0].color, [1.0; 4], "man must invert");
    assert_eq!(glyphs[1].color, [1.0; 4], "woman must invert");
    assert_eq!(
        glyphs[2].color,
        [1.0, 0.0, 0.0, 1.0],
        "the neighbour at col 3 must stay untouched"
    );
}

/// A COLOR glyph (emoji) in the cursor cell must NOT be tinted — a color
/// glyph samples its own RGBA, so re-coloring flashes it to the cursor bg.
#[test]
fn color_glyph_is_not_tinted_by_the_cursor() {
    let mut glyphs = [Primitive {
        pos: Vec2::new(34.8, 2.0),
        size: Vec2::new(32.3, 32.3),
        bearing_x: 0.0,
        atlas_pos: U16Vec2::new(1, 1),
        atlas_size: U16Vec2::new(32, 32),
        color: [0.0627, 0.0588, 0.0588, 1.0],
        bg_color: [0.0; 4],
        kind: PrimitiveKind::Color,
        page_index: 1,
    }];
    invert_cursor_cell(
        &mut glyphs,
        0,
        2,
        17.0,
        34.0,
        17.0,
        [1.0; 4],
        [0.0; 4],
    );
    assert_eq!(
        glyphs[0].color,
        [0.0627, 0.0588, 0.0588, 1.0],
        "a color emoji under the cursor must keep its own color"
    );
    assert_eq!(glyphs[0].bg_color, [0.0; 4]);
}

/// A right-column braille dot has a large left bearing: its INK origin sits in
/// the next cell, so the cursor match must use the PEN (`pos.x - bearing_x`).
#[test]
fn right_column_braille_dot_is_not_inverted_by_the_neighbour_cursor() {
    let mut glyphs = [Primitive {
        pos: Vec2::new(10.0, 1.0),
        size: Vec2::new(7.0, 25.0),
        bearing_x: 10.0,
        ..mark()
    }];
    invert_cursor_cell(
        &mut glyphs,
        0,
        1,
        17.0,
        34.0,
        17.0,
        [1.0; 4],
        [0.0; 4],
    );
    assert_eq!(
        glyphs[0].color,
        [1.0, 0.0, 0.0, 1.0],
        "⠸ at cell 0 must not be inverted by the cell-1 cursor"
    );
    assert_eq!(glyphs[0].bg_color, [0.0; 4]);
}
