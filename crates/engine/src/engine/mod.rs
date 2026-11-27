// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Public rendering API: `TextEngine` trait + `Primitive`/`PrimitiveKind`.

mod geometry;
mod input;
mod lifecycle;
mod rendering;
mod selection;

pub use geometry::Geometry;
pub use input::{Input, KeyEvent};
pub use lifecycle::Lifecycle;
pub use rendering::{GlyphBatch, Rendering};
pub use selection::{CellPoint, Selection};

/// Re-exported so the GPU pipeline keeps one import.
pub use jetfish_text::{Primitive, PrimitiveKind};

/// The engine: a marker over the five facets (Rendering, Input, Geometry,
/// Lifecycle, Selection).
pub trait TextEngine<K: Default, E>:
    Rendering + Input<Keyboard = K, Event = E> + Geometry + Lifecycle + Selection
{
}

impl<K: Default, E, T> TextEngine<K, E> for T where
    T: Rendering
        + Input<Keyboard = K, Event = E>
        + Geometry
        + Lifecycle
        + Selection
        + ?Sized
{
}
