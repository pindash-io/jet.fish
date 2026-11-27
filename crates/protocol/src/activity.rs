// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Shell activity state (OSC 133 A/C/D + OSC 9;4).

/// The shell's running/idle state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Activity {
    /// No shell-integration signal seen yet.
    #[default]
    Unknown,
    /// A command is running (OSC 133 `C`, or OSC 9;4 indeterminate/percent).
    Running,
    /// The shell is idle (OSC 133 `A`/`D`, or OSC 9;4 done/error/hidden).
    Idle,
}

impl Activity {
    /// Whether a command is currently running.
    #[must_use]
    pub const fn is_running(self) -> bool {
        matches!(self, Self::Running)
    }
}
