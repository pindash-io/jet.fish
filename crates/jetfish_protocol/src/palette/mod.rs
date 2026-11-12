// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The terminal color system: the dynamic [`Palette`], the base16/xterm color
//! tables, the color256 CIELAB generation, the shared [`Theme`], and the SGR
//! color-resolution pipeline.

#![expect(
    clippy::cast_possible_truncation,
    reason = "Terminal color math truncates channel/cell values where the range is known-safe"
)]

pub mod base16;
pub mod cielab;
pub mod color;
pub mod resolve;
pub mod state;
pub mod theme;

pub use base16::{DEFAULT_BG, DEFAULT_FG, ansi_palette, base16_palette};
pub use cielab::Cielab;
pub use color::{generate_256_palette, standard_xterm_extended};
pub use resolve::{resolve_colors, resolve_rgba};
pub use state::{DefaultColors, Palette};
pub use theme::{Defaults, Highlight, Theme, color_scheme};
