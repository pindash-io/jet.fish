// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `jetfish_command` — the decoded command model plus the apply contract. The
//! byte parser decodes into a [`Command`], a screen applies it via
//! [`Apply::apply`]; the crate owns the parsers, codec, [`Commands`] buffer,
//! and [`Recording`] log.

#![warn(missing_debug_implementations)]
#![deny(missing_docs)]
#![expect(
    clippy::cast_possible_truncation,
    reason = "Terminal cell/pixel math truncates widths and coordinates to fixed-size types where the range is known-safe"
)]
#![expect(
    clippy::allow_attributes_without_reason,
    reason = "Legacy allow attributes predate the reason convention"
)]

pub mod command;
pub mod notification;
pub use command::codec::{DecodeSeq, EncodeSeq};
pub use command::{
    ColorCommand, Command, Csi, CsiCtx, Dcs, DefaultColor, Esc, Hyperlink,
    Move, Osc, Osc52, OscCommand, Query, Sgr, ShellPrompt, WindowCommand,
    cwd_path,
};
pub use notification::{Notification, parse_osc99, parse_osc777};

#[allow(missing_docs)]
pub mod commands;
pub use commands::{Apply, Commands};

#[allow(missing_docs)]
pub mod query;
pub use query::QueryContext;
