// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Geometry facet: scroll extent, grid size, resize, viewport mapping.

use glam::{U16Vec2, UVec2, Vec2};

use jetfish_protocol::ScrollAnchor;

use super::Rendering;

/// Scroll extent, grid size, resize, and viewport mapping.
pub trait Geometry: Rendering {
    /// Total scrollable height.
    fn full_height(&self) -> f32 {
        0.0
    }

    /// The raw buffer height (every line, including trailing blanks) for the
    /// bottom-anchor `content_overflows` flag.
    fn raw_full_height(&self) -> f32 {
        self.full_height()
    }

    /// Whether the alternate buffer is active.
    fn is_alt(&self) -> bool {
        false
    }

    /// Whether a main-screen TUI (DEC 2026 synchronized) is active.
    fn is_main_tui(&self) -> bool {
        false
    }

    /// Whether a TUI just exited and the anchor is held so the shell's
    /// prompt redraw doesn't re-anchor the content down.
    fn post_tui(&self) -> bool {
        false
    }

    /// The cursor's absolute row across the full scrollback (archived +
    /// in-buffer), in cell units.
    fn cursor_abs_row(&self) -> f32 {
        0.0
    }

    /// Preferred viewport size.
    fn preferred_size(&self) -> UVec2 {
        UVec2::ZERO
    }

    /// Handle a window resize.
    fn resize(&mut self, _size: Vec2) {}

    /// The scroll anchor during a resize.
    fn resize_anchor(&self) -> ScrollAnchor {
        ScrollAnchor::Auto
    }

    /// Map a local pixel to a cell.
    fn cell_at(&self, local: Vec2, scroll_px: f32) -> U16Vec2 {
        let cell_size = self.cell_size().max(Vec2::ONE);
        let cell =
            jetfish_protocol::px_to_cells_floor(local, cell_size).as_u16vec2();
        let scroll_lines = (scroll_px / cell_size.y).floor().max(0.0) as u16;
        cell.saturating_add(U16Vec2::new(0, scroll_lines))
    }

    /// The cursor's cell.
    fn cursor_cell(&self) -> U16Vec2 {
        U16Vec2::ZERO
    }

    /// The cursor's width in cells.
    fn cursor_width(&self) -> u16 {
        1
    }
}
