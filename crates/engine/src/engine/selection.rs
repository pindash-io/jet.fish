// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Selection facet: word and line bounds, copy-on-select, OSC 8 links.

pub use jetfish_protocol::CellPoint;

/// Word and line bounds, copy-on-select, and OSC 8 links.
pub trait Selection {
    /// Word bounds for double-click.
    fn semantic_word_bounds(
        &self,
        _col: u16,
        _abs_line: usize,
    ) -> Option<(u16, u16)> {
        None
    }

    /// Logical-line bounds for triple-click.
    fn semantic_line_bounds(&self, abs_line: usize) -> (usize, usize) {
        (abs_line, abs_line.saturating_add(1))
    }

    /// The OSC 8 link at a point.
    fn hyperlink_at(
        &self,
        _local_x: f32,
        _local_y: f32,
        _scroll_px: f32,
    ) -> Option<(String, Option<String>)> {
        None
    }

    /// The absolute line at the viewport top.
    fn viewport_start_line(&self, _scroll_px: f32) -> usize {
        0
    }

    /// Map a physical cell to a logical anchor.
    fn logical_anchor(&self, cell: CellPoint) -> (u32, usize) {
        (cell.col as u32, cell.line)
    }

    /// Map a logical anchor to a physical cell.
    fn physical_anchor(&self, logical: (u32, usize)) -> CellPoint {
        CellPoint { col: logical.0 as u16, line: logical.1 }
    }

    /// The selected text.
    fn selection_text(
        &self,
        _anchor: CellPoint,
        _active: CellPoint,
    ) -> String {
        String::new()
    }

    /// The rectangular-selection text.
    fn selection_text_rectangle(
        &self,
        anchor: CellPoint,
        active: CellPoint,
    ) -> String {
        self.selection_text(anchor, active)
    }

    /// The last command's output.
    fn command_output_text(&self) -> Option<String> {
        None
    }

    /// The last command typed.
    fn command_text(&self) -> Option<String> {
        None
    }
}
