// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

#[test]
fn high_low_limits_clamp_to_region_or_screen() {
    // HIGH limit (down/right): at/inside the region → stop at the edge;
    // past the region → run to the screen edge.
    assert_eq!(high_limit(5, 10, 23), 10);
    assert_eq!(high_limit(12, 10, 23), 23);
    assert_eq!(high_limit(3, 9, 79), 9); // right: inside → margin
    assert_eq!(high_limit(10, 9, 79), 79); // right: outside → last col
    // LOW limit (up/left): at/inside the region → stop at the edge;
    // before the region → run to the screen edge.
    assert_eq!(low_limit(15, 10, 0), 10);
    assert_eq!(low_limit(8, 10, 0), 0);
    assert_eq!(low_limit(5, 4, 0), 4); // left: inside → margin
    assert_eq!(low_limit(3, 4, 0), 0); // left: outside → col 0
}

#[test]
fn clamp_to_grid_clamps_both_axes() {
    // Inside the grid: unchanged.
    assert_eq!(
        clamp_to_grid(CursorPos::new(6, 4), Grid::new(80, 24)).to_array(),
        [6, 4]
    );
    // Past the edges: clamp to the last column / row.
    assert_eq!(
        clamp_to_grid(CursorPos::new(101, 101), Grid::new(80, 24)).to_array(),
        [80, 24]
    );
    // Degenerate grid: clamp to (1, 1).
    assert_eq!(
        clamp_to_grid(CursorPos::new(5, 5), Grid::new(0, 0)).to_array(),
        [1, 1]
    );
}

#[test]
fn scroll_bounds_offsets_and_clamps() {
    // Main screen with scrollback: viewport region offset by nlines-rows.
    let sb = scroll_bounds(30, 24, false, ScrollRegion::new(1, 22));
    assert_eq!(sb.to_array(), [7, 28]); // base 6 + [1, 22]
    // Fixed grid: no offset.
    assert_eq!(
        scroll_bounds(24, 24, true, ScrollRegion::new(1, 22)).to_array(),
        [1, 22]
    );
    // Degenerate region (top > bottom) collapses to the top row.
    assert_eq!(
        scroll_bounds(24, 24, true, ScrollRegion::new(10, 5)).to_array(),
        [10, 10]
    );
}

#[test]
fn is_full_viewport_detects_the_default_region() {
    // Full viewport: top 0, bottom rows-1.
    assert!(is_full_viewport(ScrollRegion::new(0, 23), 24));
    // Partial region (top > 0 or bottom < rows-1): not full.
    assert!(!is_full_viewport(ScrollRegion::new(1, 23), 24));
    assert!(!is_full_viewport(ScrollRegion::new(0, 22), 24));
    // Degenerate rows == 0: trivially full.
    assert!(is_full_viewport(ScrollRegion::new(0, 0), 0));
}

#[test]
fn origin_offset_is_relative_to_the_origin() {
    // No offset: unchanged.
    assert_eq!(origin_offset(CursorPos::new(5, 3), 0, 0).to_array(), [5, 3]);
    // Offset by scroll top 2 (0-based) and left margin 4: subtract both.
    assert_eq!(origin_offset(CursorPos::new(10, 8), 2, 4).to_array(), [6, 6]);
    // Clamps to at least (1, 1) when the cursor is above/left of the origin.
    assert_eq!(origin_offset(CursorPos::new(2, 2), 5, 5).to_array(), [1, 1]);
}

#[test]
fn clamp_pair_rejects_degenerate() {
    // Valid pair: the high bound is clamped to `max`.
    assert_eq!(clamp_pair(1, 22, 23).unwrap().to_array(), [1, 22]);
    assert_eq!(clamp_pair(1, 99, 23).unwrap().to_array(), [1, 23]);
    assert_eq!(clamp_pair(4, 79, 79).unwrap().to_array(), [4, 79]);
    // Degenerate (a >= b): rejected.
    assert!(clamp_pair(10, 10, 23).is_none());
    assert!(clamp_pair(10, 5, 23).is_none());
    assert!(clamp_pair(6, 5, 79).is_none());
}

#[test]
fn origin_home_is_viewport_or_origin() {
    // No DECOM: viewport top-left.
    assert_eq!(origin_home(2, 4, false, 6).to_array(), [0, 6]);
    // DECOM: scroll-region top / left margin.
    assert_eq!(origin_home(2, 4, true, 6).to_array(), [4, 8]);
}

#[test]
fn inside_margins_gates_on_declrmm() {
    let m = Margins::new(2, 6);
    // DECLRMM off: always inside (a stale margin must not gate).
    assert!(inside_margins(false, 0, m));
    assert!(inside_margins(false, 10, m));
    // DECLRMM on: inside the [2, 6] rectangle only.
    assert!(inside_margins(true, 2, m));
    assert!(inside_margins(true, 5, m));
    assert!(!inside_margins(true, 1, m));
    assert!(!inside_margins(true, 7, m));
}

#[test]
fn line_feed_disposition_is_pure() {
    // Not at the bottom: move down (grow is irrelevant).
    assert_eq!(line_feed(false, true, false), VerticalMotion::Move);
    assert_eq!(line_feed(false, false, true), VerticalMotion::Move);
    // At the bottom, outside margins: no-op (DEC STD 070).
    assert_eq!(line_feed(true, false, false), VerticalMotion::Noop);
    assert_eq!(line_feed(true, false, true), VerticalMotion::Noop);
    // At the bottom, inside: grow on the main full region, else scroll.
    assert_eq!(line_feed(true, true, true), VerticalMotion::Grow);
    assert_eq!(line_feed(true, true, false), VerticalMotion::Scroll);
}

#[test]
fn reverse_index_disposition_is_pure() {
    // Not at the top: move up.
    assert_eq!(reverse_index(false, false), VerticalMotion::Move);
    // At the top, outside margins: no-op.
    assert_eq!(reverse_index(true, false), VerticalMotion::Noop);
    // At the top, inside: scroll down.
    assert_eq!(reverse_index(true, true), VerticalMotion::Scroll);
}

#[test]
fn px_to_cells_rounding_variants_clamp_and_guard() {
    let cell = Vec2::new(9.0, 18.0);
    // ceil: a sub-cell pixel rounds up to the next whole cell.
    assert_eq!(px_to_cells(Vec2::new(1.0, 1.0), cell).to_array(), [1, 1]);
    // floor: the cell containing the pixel, clamped to 0.
    assert_eq!(
        px_to_cells_floor(Vec2::new(8.0, 17.0), cell).to_array(),
        [0, 0]
    );
    assert_eq!(
        px_to_cells_floor(Vec2::new(9.0, 18.0), cell).to_array(),
        [1, 1]
    );
    // round: the nearest cell (float jitter lands on the intended cell).
    assert_eq!(
        px_to_cells_round(Vec2::new(8.9, 17.9), cell).to_array(),
        [1, 1]
    );
    assert_eq!(
        px_to_cells_round(Vec2::new(0.1, 0.1), cell).to_array(),
        [0, 0]
    );
    // Negative pixels clamp to 0 — never a negative cell.
    assert_eq!(
        px_to_cells_floor(Vec2::new(-1.0, -1.0), cell).to_array(),
        [0, 0]
    );
    assert_eq!(
        px_to_cells_round(Vec2::new(-1.0, -1.0), cell).to_array(),
        [0, 0]
    );
    // A zero cell size is guarded to 1 so the divide never panics.
    assert_eq!(
        px_to_cells(Vec2::new(5.0, 5.0), Vec2::ZERO).to_array(),
        [5, 5]
    );
}
