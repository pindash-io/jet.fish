// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `jetfish_screen` — the main/alt dual-buffer [`Screen`] shell and the
//! [`ScreenShared`] context, embedded by `jetfish_cell`'s `CellScreen`.

#![warn(missing_debug_implementations)]
#![deny(missing_docs)]

pub mod input_state;
pub use input_state::{KittyKeyboard, Mouse};

mod buffer;
pub use buffer::BufferState;

mod screen;
pub use screen::{Screen, State, TermInput};
mod shared;
pub use shared::{Clipboard, ImageState, ScreenShared, WindowTitle};

mod term;
pub use term::DEFAULT_FLAGS;
