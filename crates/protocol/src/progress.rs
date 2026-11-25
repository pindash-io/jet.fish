// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `OSC 9;4` progress-bar state.

/// `OSC 9;4` progress-bar state, as used by modern terminal emulators.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Progress {
    /// No progress bar is shown.
    #[default]
    Hidden,
    /// `OSC 9;4;0` — begin, indeterminate; the terminal animates it.
    Indeterminate,
    /// `OSC 9;4;1;<n>` — determinate progress, percent `0..=100`.
    Percent(u8),
    /// `OSC 9;4;2` — completed.
    Done,
    /// `OSC 9;4;3` — errored.
    Error,
}
