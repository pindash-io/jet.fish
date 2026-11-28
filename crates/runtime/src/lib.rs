// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `jetfish_runtime` — the terminal runtime: the generic `Term` shell,
//! PTY/input encoding, and the backend registry.
#![expect(
    missing_debug_implementations,
    reason = "Terminal state structs are large"
)]

pub mod pty;
pub mod term;
pub mod termio;
mod utils;

pub use term::{Cadence, Term, TermExt};
pub use termio::register;
pub use utils::paste_bytes;
