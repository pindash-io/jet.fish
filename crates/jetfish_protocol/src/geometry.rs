// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Shared 2D viewport / scroll-region / margin geometry for the line and cell
//! screens. Both screens compute the same DEC geometry (viewport, DECSTBM
//! scroll region, DECSLRM margins, DECOM origin), so these are pure,
//! unit-tested free functions over the screen dimensions.
//!
//! Columns are 0-based inclusive; rows are 0-based absolute (cell screen) or
//! 1-based viewport (line screen). 2D bounds use the [`USizeVec2`] aliases
//! ([`Grid`], [`ScrollRegion`], [`Margins`], [`CursorPos`]).

use crate::CursorPos;
use crate::Grid;
use crate::Margins;
use crate::ScrollRegion;
use crate::USizeVec2;

/// The absolute index of the first visible line.
///
/// The main screen keeps scrollback above the visible grid, so its viewport
/// starts at `nlines - rows`; a fixed grid (alt screen) or unbounded growth
/// (`rows == 0`) starts at line `0`.
#[inline]
pub const fn viewport_start(nlines: usize, rows: usize, fixed: bool) -> usize {
    if fixed || rows == 0 { 0 } else { nlines.saturating_sub(rows) }
}

/// Map a 0-based **viewport row** to an **absolute line index**: the row is
/// offset by the viewport start (the scrollback), clamped to the last line.
#[inline]
pub fn viewport_row(
    nlines: usize,
    rows: usize,
    fixed: bool,
    row: usize,
) -> usize {
    (viewport_start(nlines, rows, fixed) + row).min(nlines.saturating_sub(1))
}

/// Resolve the **absolute** `[top, bottom]` bounds of the DECSTBM scroll
/// region.
///
/// `scroll` is the 0-based viewport-relative region; on the main screen it is
/// offset by the scrollback. Both bounds are clamped to `[0, last]`, and a
/// degenerate region (`top > bottom`) collapses to the top row so the
/// `y >= x` invariant always holds.
#[inline]
pub fn scroll_bounds(
    nlines: usize,
    rows: usize,
    fixed: bool,
    scroll: ScrollRegion,
) -> ScrollRegion {
    let last = nlines.saturating_sub(1);
    let base = if fixed { 0 } else { nlines.saturating_sub(rows.max(1)) };
    let v = (ScrollRegion::splat(base) + scroll)
        .clamp(ScrollRegion::ZERO, ScrollRegion::splat(last));
    ScrollRegion::new(v.x, v.y.max(v.x))
}

/// The 0-based inclusive limit for cursor motion toward **higher** indices
/// (down or right).
///
/// xterm CUD/CUF: when the cursor is at/before the
/// DECSTBM/DECSLRM region edge it stops there; past the edge it runs to the
/// screen's last line/column.
#[inline]
pub const fn high_limit(pos: usize, region: usize, screen: usize) -> usize {
    if pos <= region { region } else { screen }
}

/// The 0-based inclusive limit for cursor motion toward **lower** indices
/// (up or left).
///
/// xterm CUU/CUB: when the cursor is at/after the
/// DECSTBM/DECSLRM region edge it stops there; before the edge it runs to the
/// screen's first line/column.
#[inline]
pub const fn low_limit(pos: usize, region: usize, screen: usize) -> usize {
    if pos >= region { region } else { screen }
}

/// Clamp a 0-based cursor COLUMN to the grid's right edge `[0, last_col]`.
/// Cursor columns are `usize` (never negative), so this is the upper clamp
/// (`col.min(last_col)`); the named helper documents the intent at the
/// CUP/CHA/DECSTBM/scroll call sites (the cell screen's `x.min(last_col)`).
#[inline]
pub fn clamp_col(col: usize, last_col: usize) -> usize {
    col.min(last_col)
}

/// Clamp a 0-based cursor ROW to the grid's bottom edge `[0, last_row]`.
/// Cursor rows are `usize` (never negative), so this is the upper clamp;
/// the named helper documents the intent at the CUP/scroll/DECRC call sites
/// (the cell screen's `y.min(last_line_index)`).
#[inline]
pub fn clamp_row(row: usize, last_row: usize) -> usize {
    row.min(last_row)
}

/// Whether the DECSTBM scroll region covers the **whole viewport**.
///
/// `top == 0` and `bottom >= rows - 1` (0-based inclusive) is the power-on
/// default region; the streaming scrollback fast path keys off it.
#[inline]
pub const fn is_full_viewport(scroll: ScrollRegion, rows: usize) -> bool {
    scroll.x == 0 && scroll.y >= rows.saturating_sub(1)
}

/// Clamp a 0-based inclusive pair `(a, b)` to `[0, max]`, returning `None` for
/// a degenerate pair (`a >= b`). Both DECSTBM scroll regions and DECSLRM
/// margins reject a degenerate region and clamp their high bound.
#[inline]
pub fn clamp_pair(a: usize, b: usize, max: usize) -> Option<USizeVec2> {
    let b = b.min(max);
    (a < b).then_some(USizeVec2::new(a, b))
}

/// Whether a 0-based cursor column is OUTSIDE the DECSLRM horizontal margins.
///
/// The DEC STD 070 gate: LF/RI at the scroll-region edge do nothing when the
/// cursor column is outside `[margin.x, margin.y]`.
#[inline]
pub const fn outside_margins(col0: usize, margin: Margins) -> bool {
    col0 < margin.x || col0 > margin.y
}

/// Whether a LF/RI at the DECSTBM region boundary may proceed given the
/// DECSLRM horizontal margins.
///
/// With DECLRMM off the margins are conceptually full-width, but `margin` is
/// NOT refreshed on resize while the mode is off, so a stale `margin` must not
/// gate the motion — `margin_active` (`lrm_mode`) is required and this returns
/// `true` whenever it is false. With DECLRMM on the region is the rectangle
/// `[left..=right] × [top..=bottom]` and the cursor column must be inside
/// `[margin.x, margin.y]` (DEC STD 070 / xterm).
#[inline]
pub const fn inside_margins(
    margin_active: bool,
    col0: usize,
    margin: Margins,
) -> bool {
    !margin_active || !outside_margins(col0, margin)
}

/// The disposition of a vertical cursor motion (line-feed or reverse-index):
/// what the caller must do to its own buffer.
///
/// The DEC STD 070 decision is pure math shared by the line and cell screens;
/// only the buffer mutation (string vs fixed grid) is screen-specific. `Grow`
/// is line-feed-only: the main screen's full-viewport region accumulates
/// scrollback instead of scrolling.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VerticalMotion {
    /// Grow scrollback (main screen, full-viewport region; LF only).
    Grow,
    /// Scroll the region (up for LF, down for RI); the cursor stays at the
    /// region boundary.
    Scroll,
    /// Move the cursor one row (down for LF, up for RI), clamping at the
    /// region boundary.
    Move,
    /// Nothing: the cursor sits at the region boundary but OUTSIDE the
    /// DECSLRM horizontal margins (DEC STD 070).
    Noop,
}

/// Line-feed (LF / IND / VT / FF) disposition.
///
/// `at_bottom` is the caller's "cursor at the region bottom" test: the
/// streaming line screen is bottom-anchored (`row >= bottom`), the fixed cell
/// grid uses `row == bottom` and clamps below-region motion itself. `inside`
/// is [`inside_margins`]; `grow` is `is_full_viewport(scroll, rows) &&
/// !is_alt` — the main screen's full-viewport region grows scrollback.
#[inline]
pub const fn line_feed(
    at_bottom: bool,
    inside: bool,
    grow: bool,
) -> VerticalMotion {
    if !at_bottom {
        return VerticalMotion::Move;
    }
    if !inside {
        return VerticalMotion::Noop;
    }
    if grow {
        return VerticalMotion::Grow;
    }
    VerticalMotion::Scroll
}

/// Reverse-index (RI) disposition. `at_top` is the caller's "cursor at the
/// region top" test (line screen: `row <= top`; cell grid: `row == top`).
/// RI never grows scrollback, so it yields `Scroll`/`Move`/`Noop` only.
#[inline]
pub const fn reverse_index(at_top: bool, inside: bool) -> VerticalMotion {
    if at_top {
        if inside { VerticalMotion::Scroll } else { VerticalMotion::Noop }
    } else {
        VerticalMotion::Move
    }
}

/// Offset a 1-based viewport-relative cursor `(col, row)` by the DECOM origin
/// — the DECSTBM scroll-region top and the DECSLRM left margin (both 0-based)
/// — clamped to at least `(1, 1)`: the DSR `CSI 6n` position under origin
/// mode.
#[inline]
pub fn origin_offset(
    pos: CursorPos,
    scroll_top: usize,
    margin_left: usize,
) -> CursorPos {
    CursorPos::new(
        pos.x.saturating_sub(margin_left).max(1),
        pos.y.saturating_sub(scroll_top).max(1),
    )
}

/// Clamp a 1-based cursor position to the grid `[1, cols] × [1, rows]`
/// (inclusive) with one vectorized `min` against `grid.max(ONE)` — the
/// SIMD-friendly form of the grid-bounds check; a degenerate `grid` of `(0, 0)`
/// clamps to `(1, 1)`.
#[inline]
pub fn clamp_to_grid(pos: CursorPos, grid: Grid) -> CursorPos {
    pos.min(grid.max(CursorPos::ONE))
}

/// The cursor home position after a DECSTBM/DECSLRM change: the viewport
/// top-left normally, or the origin
/// (scroll-region top / left margin) under DECOM. Returns 0-based absolute
/// `(col, row)`.
#[inline]
pub const fn origin_home(
    scroll_top: usize,
    margin_left: usize,
    decom: bool,
    viewport_start: usize,
) -> CursorPos {
    if decom {
        CursorPos::new(margin_left, viewport_start + scroll_top)
    } else {
        CursorPos::new(0, viewport_start)
    }
}

pub fn clamp_rect_to_page(
    top: usize,
    left: usize,
    bottom: usize,
    right: usize,
    cols: usize,
    rows: usize,
) -> (usize, usize, usize, usize) {
    let cols = cols.max(1);
    let rows = rows.max(1);
    (top.min(rows), left.min(cols), bottom.min(rows), right.min(cols))
}

#[cfg(test)]
mod tests {
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
            clamp_to_grid(CursorPos::new(101, 101), Grid::new(80, 24))
                .to_array(),
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
        assert_eq!(
            origin_offset(CursorPos::new(5, 3), 0, 0).to_array(),
            [5, 3]
        );
        // Offset by scroll top 2 (0-based) and left margin 4: subtract both.
        assert_eq!(
            origin_offset(CursorPos::new(10, 8), 2, 4).to_array(),
            [6, 6]
        );
        // Clamps to at least (1, 1) when the cursor is above/left of the origin.
        assert_eq!(
            origin_offset(CursorPos::new(2, 2), 5, 5).to_array(),
            [1, 1]
        );
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
}
