// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! 2D viewport / scroll-region / margin geometry as pure, unit-tested free
//! functions. Columns are 0-based inclusive; rows are 0-based absolute (cell)
//! or 1-based viewport (line).

use glam::{UVec2, Vec2};

use crate::CursorPos;
use crate::Grid;
use crate::Margins;
use crate::ScrollRegion;
use crate::USizeVec2;

/// The cell grid → physical pixels (no rounding; the host rounds to the
/// nearest pixel for the window size).
#[inline]
#[must_use]
pub fn cells_to_px(cells: UVec2, cell: Vec2) -> Vec2 {
    cells.as_vec2() * cell
}

/// Physical pixels → the next whole cell (ceil), guarding a sub-cell viewport
/// so the grid never divides by zero. The shared viewport-culling / cell-
/// extent conversion.
#[inline]
#[must_use]
pub fn px_to_cells(px: Vec2, cell: Vec2) -> UVec2 {
    (px / cell.max(Vec2::splat(1.0))).ceil().as_uvec2()
}

/// Physical pixels → the cell containing them (floor), clamped to `≥ 0`. The
/// grid-size / hit-test variant (the grid never exceeds the viewport).
#[inline]
#[must_use]
pub fn px_to_cells_floor(px: Vec2, cell: Vec2) -> UVec2 {
    (px / cell.max(Vec2::splat(1.0))).floor().max(Vec2::ZERO).as_uvec2()
}

/// Physical pixels → the NEAREST cell (round), clamped to `≥ 0`. The cursor
/// hit-test variant (float jitter lands on the intended cell).
#[inline]
#[must_use]
pub fn px_to_cells_round(px: Vec2, cell: Vec2) -> UVec2 {
    (px / cell.max(Vec2::splat(1.0))).round().max(Vec2::ZERO).as_uvec2()
}

/// The disposition of a vertical cursor motion (line-feed or reverse-index):
/// what the caller must do to its own buffer.
///
/// The DEC STD 070 decision is pure math; only the buffer mutation (string
/// vs fixed grid) is screen-specific. `Grow` is line-feed-only: the main
/// screen's full-viewport region accumulates scrollback instead of scrolling.
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

/// The absolute index of the first visible line: `nlines - rows` on the main
/// screen (scrollback above), `0` on a fixed grid.
#[inline]
pub const fn viewport_start(nlines: usize, rows: usize, fixed: bool) -> usize {
    if fixed || rows == 0 { 0 } else { nlines.saturating_sub(rows) }
}

/// Map a viewport row to an absolute line index (offset by scrollback,
/// clamped to the last line).
#[must_use]
#[inline]
pub fn viewport_row(
    nlines: usize,
    rows: usize,
    fixed: bool,
    row: usize,
) -> usize {
    (viewport_start(nlines, rows, fixed) + row).min(nlines.saturating_sub(1))
}

/// Resolve the absolute DECSTBM `[top, bottom]` bounds: offset by scrollback
/// on the main screen, clamped to `[0, last]`; a degenerate region collapses
/// to the top row.
#[must_use]
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

/// The inclusive limit for motion toward higher indices (down/right): the
/// region edge when at/inside it, else the screen edge.
#[inline]
pub const fn high_limit(pos: usize, region: usize, screen: usize) -> usize {
    if pos <= region { region } else { screen }
}

/// The inclusive limit for motion toward lower indices (up/left): the region
/// edge when at/after it, else the screen edge.
#[inline]
pub const fn low_limit(pos: usize, region: usize, screen: usize) -> usize {
    if pos >= region { region } else { screen }
}

/// Clamp a cursor column to `[0, last_col]` (columns are `usize`, so only
/// the upper bound clamps).
#[must_use]
#[inline]
pub fn clamp_col(col: usize, last_col: usize) -> usize {
    col.min(last_col)
}

/// Clamp a cursor row to `[0, last_row]`.
#[must_use]
#[inline]
pub fn clamp_row(row: usize, last_row: usize) -> usize {
    row.min(last_row)
}

/// Whether the scroll region covers the whole viewport (`top == 0 &&
/// bottom >= rows - 1`); the streaming scrollback fast path keys off this.
#[inline]
pub const fn is_full_viewport(scroll: ScrollRegion, rows: usize) -> bool {
    scroll.x == 0 && scroll.y >= rows.saturating_sub(1)
}

/// Clamp `(a, b)` to `[0, max]`; `None` when `a >= b` (a degenerate region).
#[must_use]
#[inline]
pub fn clamp_pair(a: usize, b: usize, max: usize) -> Option<USizeVec2> {
    let b = b.min(max);
    (a < b).then_some(USizeVec2::new(a, b))
}

/// Whether a cursor column is outside the DECSLRM horizontal margins.
#[inline]
pub const fn outside_margins(col0: usize, margin: Margins) -> bool {
    col0 < margin.x || col0 > margin.y
}

/// Whether LF/RI may proceed given the DECSLRM margins. With DECLRMM off a
/// stale `margin` must not gate the motion, so `margin_active` is required and
/// this returns `true` when it is false.
#[inline]
pub const fn inside_margins(
    margin_active: bool,
    col0: usize,
    margin: Margins,
) -> bool {
    !margin_active || !outside_margins(col0, margin)
}

/// Line-feed (LF / IND / VT / FF) disposition. `at_bottom` is the caller's
/// "cursor at the region bottom" test; `inside` is [`inside_margins`]; `grow`
/// is the main screen's full-viewport region.
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

/// Reverse-index disposition. `at_top` is the caller's "cursor at the region
/// top" test; RI never grows scrollback.
#[inline]
pub const fn reverse_index(at_top: bool, inside: bool) -> VerticalMotion {
    match (at_top, inside) {
        (true, true) => VerticalMotion::Scroll,
        (true, false) => VerticalMotion::Noop,
        (false, _) => VerticalMotion::Move,
    }
}

/// Offset a 1-based cursor by the DECOM origin (scroll top + left margin),
/// clamped to at least `(1, 1)`.
#[must_use]
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

/// Clamp a 1-based cursor to the grid `[1, cols] × [1, rows]`; a degenerate
/// `(0, 0)` grid clamps to `(1, 1)`.
#[must_use]
#[inline]
pub fn clamp_to_grid(pos: CursorPos, grid: Grid) -> CursorPos {
    pos.min(grid.max(CursorPos::ONE))
}

/// The cursor home after a DECSTBM/DECSLRM change: viewport top-left, or the
/// origin (scroll top / left margin) under DECOM. Returns 0-based absolute
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

/// Clamp a 0-based inclusive DECRQCRA rectangle to the page size.
#[must_use]
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
#[path = "../tests/geometry/mod.rs"]
mod tests;
