// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Kitty graphics protocol (`DCS q` / `APC _G`).

mod decode;
mod meta;
mod parser;

pub use decode::{decode_kitty_file, decode_kitty_raw, decode_kitty_rgba};
pub use meta::{KittyImage, KittyMeta, parse_kitty_meta};
pub use parser::KittyParser;

#[cfg(test)]
#[path = "../../tests/kitty/mod.rs"]
mod tests;
