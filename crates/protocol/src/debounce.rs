// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Runtime-tunable input/layout debounce knobs.

use serde::Deserialize;

/// Quiet gap after which a burst is interactive and repaints immediately.
pub const INTERACTIVE_GAP_MS: u64 = 50;

/// A burst this small is a key echo (a keypress or a few repeats) — always
/// interactive, even with no gap.
pub const ECHO_BYTES: usize = 64;

/// Streaming relayout interval for the main screen. One frame (~60fps).
pub const STREAM_RELAYOUT_MS: u64 = 16;

/// Streaming relayout interval for the alt screen.
pub const ALT_RELAYOUT_MS: u64 = 16;

/// Input/layout debounce knobs, overridable once at startup.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(default = "Debounce::default")]
pub struct Debounce {
    /// Interactive-input gap threshold (ms).
    pub interactive_gap_ms: u64,
    /// Burst size (bytes) below which input is always interactive.
    pub echo_bytes: usize,
    /// Continuous-streaming relayout interval, main screen (ms).
    pub stream_relayout_ms: u64,
    /// Continuous-streaming relayout interval, alt screen (ms).
    pub alt_relayout_ms: u64,
}

impl Debounce {
    /// The compiled default debounce knobs.
    pub const DEFAULT: Self = Self {
        interactive_gap_ms: INTERACTIVE_GAP_MS,
        echo_bytes: ECHO_BYTES,
        stream_relayout_ms: STREAM_RELAYOUT_MS,
        alt_relayout_ms: ALT_RELAYOUT_MS,
    };

    /// A burst is interactive after a quiet gap or when it is a small key
    /// echo; everything else is a continuous stream.
    #[allow(
        clippy::cast_possible_truncation,
        reason = "a Duration gap is far below u64::MAX ms"
    )]
    pub const fn is_interactive(
        &self,
        gap: std::time::Duration,
        drained_bytes: usize,
    ) -> bool {
        gap.as_millis() as u64 >= self.interactive_gap_ms
            || drained_bytes <= self.echo_bytes
    }
}

impl Default for Debounce {
    fn default() -> Self {
        Self::DEFAULT
    }
}
