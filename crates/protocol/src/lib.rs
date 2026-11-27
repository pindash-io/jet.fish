// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `jetfish_protocol`: the screen-agnostic protocol primitives, the cell-width
//! model, SGR spans/attributes, text helpers, tab stops, grid geometry, and
//! themes. The decoded command model (`Command`/`Apply`) lives in
//! `jetfish_command`; screen models (cell grid, string+span) implement both in
//! their own crates.
//!
//! | module | purpose |
//! | --- | --- |
//! | [`attrs`] | packed SGR attributes + flat `Style` projection |
//! | [`span`] | sorted SGR color spans |
//! | [`width`] | byte ↔ cell width walkers |
//! | [`geometry`] | viewport / scroll-region / margin math |
//! | [`tabstops`] | horizontal tab stops |
//! | [`text`] | char boundaries, line ranges, word bounds |
//! | [`uri`] | percent-decoding and OSC 7 working directories |
//! | [`palette`] | dynamic palette, base16/color256, theme |
//! | [`input`] | key/mouse-report encoding, alt-screen flags |
//! | [`hash`] | deterministic foldhash maps |

#![warn(missing_debug_implementations)]
#![deny(missing_docs)]

use glam::USizeVec2;

/// A terminal grid size in cells: `.x` = columns, `.y` = rows.
pub type Grid = USizeVec2;

/// A DECSTBM scroll region: `.x` = top, `.y` = bottom (inclusive; 0-based).
pub type ScrollRegion = USizeVec2;
/// DECSLRM horizontal margins: `.x` = left, `.y` = right (0-based inclusive).
pub type Margins = USizeVec2;
/// A 2D cell position: `.x` = 0-based column, `.y` = 0-based row/line index.
pub type CursorPos = USizeVec2;

/// A physical cell coordinate for selection/hit-testing: `(col, line)`,
/// 0-based. `line` is an ABSOLUTE scrollback index (may exceed the viewport).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CellPoint {
    /// The 0-based cell column.
    pub col: u16,
    /// The 0-based absolute line.
    pub line: usize,
}

/// Callback invoked from a reader thread when new data arrives.
pub type WakeCallback = std::sync::Arc<dyn Fn() + Send + Sync>;
/// Shared slot holding the current wake callback.
pub type WakeSlot = std::sync::Arc<std::sync::Mutex<Option<WakeCallback>>>;

pub mod attrs;
pub mod debounce;
pub mod error;
pub mod font;

pub use attrs::{
    Attrs, Style, apply_sgr_effects, bold_to_bright, reverse_colors,
};
pub use debounce::{
    ALT_RELAYOUT_MS, Debounce, ECHO_BYTES, INTERACTIVE_GAP_MS,
    STREAM_RELAYOUT_MS,
};
pub use error::{Error, Result};
pub use font::Font;

pub mod activity;
pub mod progress;
pub use activity::Activity;
pub use glam::FloatExt;
pub use progress::Progress;

pub mod deccara;
pub use deccara::{apply_rect_flags, rect_attr_mask};

pub mod span;
pub use span::{Span, Spans};

pub mod width;
pub use width::{
    EmojiTrace, SplitBoundaries, cell_col_byte,
    cell_col_byte_ceil_from_widths, cell_col_byte_from_widths,
    char_cell_width, classify_cell, decrcra_checksum,
    grapheme_end_from_widths, grapheme_tail_end,
    grapheme_tail_end_from_widths, line_cell_width, splice_offset,
    split_widths, wrap_width,
};

pub mod geometry;
pub use geometry::{
    VerticalMotion, cells_to_px, clamp_col, clamp_pair, clamp_rect_to_page,
    clamp_row, clamp_to_grid, high_limit, inside_margins, is_full_viewport,
    line_feed, low_limit, origin_home, origin_offset, outside_margins,
    px_to_cells, px_to_cells_floor, px_to_cells_round, reverse_index,
    scroll_bounds, viewport_row, viewport_start,
};

pub mod padding;
pub use padding::Padding;

pub mod tabstops;
pub use tabstops::TabStops;

pub mod text;
pub mod uri;
pub mod util;
pub use text::{
    WORD_BOUNDARIES, line_start_byte, semantic_word_bounds, text_line_ranges,
};
pub use util::{
    MAX_THREAD_OVERRIDE, env_usize, hex_decode, hex_encode, write_fd_all,
};

pub mod palette;
pub use palette::base16;
pub use palette::{
    Cielab, DEFAULT_BG, DEFAULT_FG, DefaultColors, Defaults, Highlight,
    Palette, Theme, base16_palette, color_scheme, generate_256_palette,
    resolve_colors, resolve_rgba, standard_xterm_extended,
};

pub mod input;
pub use input::{
    AltScreenSwitch, alt_screen_action, encode_mouse_report,
    select_press_encoding,
};
pub use uri::{cwd_path, percent_decode};

pub mod hash;
pub use hash::{FixedHasher, HashMap, HashSet, new_hasher};

/// The protocol primitives every screen touches, for `use
/// jetfish_protocol::prelude::*;`. Types only; free functions stay explicit.
pub mod prelude {
    pub use crate::{
        Activity, Attrs, CursorPos, EmojiTrace, Grid, Margins, Palette,
        Progress, ScrollRegion, SplitBoundaries, Style, TabStops,
    };
    pub use vtty::prelude::*;
}
