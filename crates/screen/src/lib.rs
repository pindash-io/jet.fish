// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `jetfish_screen`: the shared screen shell, the main/alt dual-buffer [`Screen`]
//! and the [`ScreenShared`] context both screen models embed.
//!
//! The cell screen itself (`CellScreen`) lives in `jetfish_cell`.
//!
//! - `screen.rs`: generic [`Screen`] shell.
//! - `shared.rs`: [`ScreenShared`] (grid, palette, OSC 52 policy, …).
//! - `buffer.rs`: per-buffer `BufferState`.
//! - `input_state.rs`: mouse and kitty-keyboard input state.

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
