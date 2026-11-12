// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `jetfish_protocol` — the screen-agnostic protocol primitives: the cell-width
//! model, SGR spans/attributes, text helpers, tab stops, grid geometry, and
//! themes. The decoded command model (`Command`/`Apply`) lives in
//! `jetfish_command`; screen models (cell grid, string+span) implement both in
//! their own crates.

#![warn(missing_debug_implementations)]
#![deny(missing_docs)]
#![expect(
    clippy::allow_attributes_without_reason,
    reason = "Legacy allow attributes predate the reason convention"
)]

// A facade over the parser + width crates: screens depend on `jetfish_protocol`
// alone and never re-declare which parser/width/palette backend they use.
/// Small-string type used by the reply and status formats.
pub use compact_str::{CompactString, format_compact};
/// Terminal grid dimensions in cells: `x = columns`, `y = rows`.
pub use glam::USizeVec2;
pub use vtty::{
    Charset, Charsets, ColorId, ColorSpec, CursorStyle, DEFAULT_BG_ID,
    DEFAULT_FG_ID, Key, KeyEncoding, KeyEventKind, KeyLocation, KeyModes,
    KeySpec, KittyFlags, Modifiers, MouseButton, MouseTracking, Scs,
    SgrEffect, SgrFlags, StyleFlags, TermFlags, UnderlineStyle, blend_faint,
    encode_key, format_osc_background, format_osc_color_reply,
    format_osc_palette_reply, indexed_rgb, legacy_ctrl, parse,
    parse_hex_color, rgb, rgba_to_u32, u32_to_rgb, u32_to_rgba,
};
pub use vtty_width::{
    Properties, cell_advance, cell_col_char_ceil_from_widths,
    cell_col_char_end_from_widths, cell_col_char_from_widths, cell_width,
    cell_width_sum, cell_widths, cell_widths_from_codepoints, cjk_fast_width,
    cjk_width, composes_precomposed, first_non_ascii, pua_fast_width,
    set_cjk_width, width_and_standalone, width_and_standalone_with,
};
/// A terminal grid size in cells: `.x` = columns, `.y` = rows.
pub type Grid = USizeVec2;

/// The four window paddings (the gap between the window edge and the text
/// grid), in logical pixels. Defaults to 10 px on every side; negative sides
/// are invalid and clamp to zero.
#[derive(Clone, Copy, Debug, PartialEq, serde::Deserialize)]
#[serde(default)]
pub struct Padding {
    /// Top padding (logical px).
    pub top: f32,
    /// Right padding (logical px).
    pub right: f32,
    /// Bottom padding (logical px).
    pub bottom: f32,
    /// Left padding (logical px).
    pub left: f32,
}

impl Padding {
    /// Zero padding on every side.
    pub const ZERO: Self =
        Self { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 };

    /// Each side clamped to non-negative.
    #[must_use]
    pub const fn clamped(self) -> Self {
        Self {
            top: self.top.max(0.0),
            right: self.right.max(0.0),
            bottom: self.bottom.max(0.0),
            left: self.left.max(0.0),
        }
    }
}

impl Default for Padding {
    fn default() -> Self {
        Self { top: 10.0, right: 10.0, bottom: 10.0, left: 10.0 }
    }
}
/// A DECSTBM scroll region: `.x` = top, `.y` = bottom (inclusive; 0-based in
/// the cell screen, 1-based in the line screen).
pub type ScrollRegion = USizeVec2;
/// DECSLRM horizontal margins: `.x` = left, `.y` = right (0-based inclusive).
pub type Margins = USizeVec2;
/// A 2D cell position: `.x` = 0-based column, `.y` = 0-based row/line index.
pub type CursorPos = USizeVec2;

/// Default terminal flags (matching the cell screen's power-on modes: cursor
/// visible, autowrap, and cursor blink).
pub const DEFAULT_FLAGS: TermFlags = TermFlags::CURSOR_VISIBLE
    .union(TermFlags::WRAPAROUND)
    .union(TermFlags::CURSOR_BLINK);

/// Callback invoked from a reader thread when new data arrives.
pub type WakeCallback = std::sync::Arc<dyn Fn() + Send + Sync>;
/// Shared slot holding the current wake callback.
pub type WakeSlot = std::sync::Arc<std::sync::Mutex<Option<WakeCallback>>>;


#[allow(missing_docs)]
pub mod attrs;
pub use attrs::{
    Attrs, Progress, Style, apply_sgr_effects, bold_is_bright, bold_to_bright,
    reverse_colors, set_bold_is_bright,
};

#[allow(missing_docs)]
pub mod deccara;
pub use deccara::{apply_rect_flags, rect_attr_mask};

#[allow(missing_docs)]
pub mod span;
pub use span::{Span, Spans};

#[allow(missing_docs)]
pub mod width;
pub use width::{
    EmojiTrace, SplitBoundaries, cell_col_byte,
    cell_col_byte_ceil_from_widths, cell_col_byte_from_widths,
    char_cell_width, classify_cell, decrcra_checksum,
    grapheme_end_from_widths, grapheme_tail_end,
    grapheme_tail_end_from_widths, line_cell_width, splice_offset,
    split_widths, wrap_width,
};

#[allow(missing_docs)]
pub mod geometry;
pub use geometry::{
    VerticalMotion, clamp_col, clamp_pair, clamp_rect_to_page, clamp_row,
    clamp_to_grid, high_limit, inside_margins, is_full_viewport, line_feed,
    low_limit, origin_home, origin_offset, outside_margins, reverse_index,
    scroll_bounds, viewport_row, viewport_start,
};

#[allow(missing_docs)]
pub mod tabstops;
pub use tabstops::TabStops;

#[allow(missing_docs)]
pub mod text;
pub mod uri;
pub mod util;
pub use text::{
    WORD_BOUNDARIES, ceil_char_boundary, floor_char_boundary, line_start_byte,
    semantic_word_bounds, text_line_ranges,
};
pub use util::{hex_decode, hex_encode, write_fd_all};

#[allow(missing_docs)]
pub mod palette;
pub use palette::base16;
pub use palette::{
    Cielab, DEFAULT_BG, DEFAULT_FG, DefaultColors, Defaults, Highlight,
    Palette, Theme, ansi_palette, base16_palette, color_scheme,
    generate_256_palette, resolve_colors, resolve_rgba,
    standard_xterm_extended,
};

#[allow(missing_docs)]
pub mod keyboard;
pub use keyboard::{AltScreenSwitch, alt_screen_action, encode_mouse_report};
pub use uri::{cwd_path, percent_decode};


#[allow(missing_docs)]
pub mod hash;
pub use hash::{FixedHasher, HashMap, HashSet, new_hasher};

#[allow(missing_docs)]
pub mod blink;
pub use blink::{
    blink_on, cursor_blink_on, reset_cursor_blink, toggle_blink,
    toggle_cursor_blink,
};

/// The protocol primitives every screen touches, for `use
/// jetfish_protocol::prelude::*;`. Types only — the free functions stay
/// explicit so call sites keep a readable origin.
pub mod prelude {
    pub use crate::{
        Attrs, ColorId, CompactString, CursorPos, CursorStyle, DEFAULT_BG_ID,
        DEFAULT_FG_ID, DEFAULT_FLAGS, EmojiTrace, Grid, Margins, MouseButton,
        MouseTracking, Palette, Progress, ScrollRegion, Scs, SgrFlags,
        SplitBoundaries, Style, StyleFlags, TabStops, TermFlags, USizeVec2,
        UnderlineStyle,
    };
}
