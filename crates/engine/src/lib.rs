// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `jetfish_engine` — the rendering contract between the screens and the
//! window: the [`TextEngine`] trait (five facets), the shared glyph/image
//! types, the [`ScreenBackend`] factory, and the shared [`tuning`].
//!
//! A leaf crate: depends only on `jetfish_text` and `jetfish_protocol`.

#![warn(missing_debug_implementations)]
#![deny(missing_docs)]
#![expect(
    clippy::cast_possible_truncation,
    reason = "Terminal cell/pixel math truncates widths and coordinates to fixed-size types where the range is known-safe"
)]

pub mod backend;
pub mod cursor;
pub mod engine;
pub mod image;
pub mod tuning;

pub use backend::{Engine, ScreenBackend, SpawnConfig};
pub use cursor::invert_cursor_cell;
pub use engine::{
    CellPoint, Geometry, Input, KeyEvent, Lifecycle, Primitive, PrimitiveKind,
    Rendering, Selection, TextEngine,
};
pub use image::ImageQuad;
