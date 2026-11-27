// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Terminal runtime-tuning knobs.

use vtty::CursorStyle;

/// Maximum scrollback lines kept in memory before archiving to the
/// disk-backed scrollback log.
pub const SCROLLBACK_LINES: usize = 5_000;

/// Floor for the ring during a burst: it may grow to 2× [`SCROLLBACK_LINES`],
/// but never below this — a tiny limit must not archive every chunk.
pub const SCROLLBACK_BURST_TRIM_MIN: usize = 1_024;

/// Batch the final trim: let the ring grow this many lines past the limit
/// before trimming (amortizes the O(ring) re-anchor + arena rebuild).
pub const SCROLLBACK_TRIM_BATCH: usize = 512;

/// Maximum disk-archived lines; once full, overflow is dropped (head
/// retention). Override with `JETFISH_SCROLLBACK_DISK_LIMIT`.
pub const SCROLLBACK_DISK_LIMIT: usize = 100_000;

/// Coalesce window for input, one frame. A one-shot burst repaints one frame
/// after it goes quiet; a continuous stream hits the throttle instead.
pub const INTERACTIVE_SETTLE_MS: u64 = 16;

/// Coalesce window for the MAIN-screen SIGWINCH (~one frame). The first
/// resize arms a one-shot timer; the child gets the LATEST size.
pub const SIGWINCH_COALESCE_MS: u64 = 25;

/// Default cursor style (vertical bar).
pub const CURSOR_STYLE: CursorStyle = CursorStyle::Bar;

/// Width of the vertical bar cursor in pixels.
pub const CURSOR_BAR_WIDTH: u16 = 2;

/// Border width of the hollow (blurred) cursor outline in pixels.
pub const CURSOR_HOLLOW_BORDER_WIDTH: u16 = 1;

/// Height of the underline cursor in pixels.
pub const CURSOR_UNDERLINE_HEIGHT: u16 = 2;

static DEBOUNCE: std::sync::OnceLock<Debounce> = std::sync::OnceLock::new();
static LINE_HEIGHT_OVERRIDE: std::sync::OnceLock<f32> =
    std::sync::OnceLock::new();

/// Runtime-tunable debounce knobs, overridable once at startup.
pub use jetfish_protocol::Debounce;

/// The process-wide disk-archive cap, parsed once from the environment.
#[must_use]
pub fn scrollback_disk_limit() -> usize {
    env_usize("JETFISH_SCROLLBACK_DISK_LIMIT", SCROLLBACK_DISK_LIMIT)
}

/// The process-wide in-memory scrollback cap, parsed from the environment
/// (falls back to [`SCROLLBACK_LINES`]).
#[must_use]
pub fn scrollback_lines() -> usize {
    env_usize("JETFISH_SCROLLBACK_LINES", SCROLLBACK_LINES)
}

/// Parse an unsigned environment override, falling back to `fallback`.
#[must_use]
fn env_usize(key: &str, fallback: usize) -> usize {
    jetfish_protocol::env_usize(key).unwrap_or(fallback)
}

/// Override the debounce knobs (call once at startup, before engines spawn).
pub fn set_debounce(d: Debounce) {
    let _ = DEBOUNCE.set(d);
}

/// The active debounce knobs (defaults, or the host's override).
#[must_use]
pub fn debounce() -> Debounce {
    *DEBOUNCE.get_or_init(Debounce::default)
}

/// Override the cell-grid line height (call once at startup, before engines
/// spawn).
pub fn set_line_height(v: f32) {
    let _ = LINE_HEIGHT_OVERRIDE.set(v);
}

/// The active cell-grid line height (defaults to the compiled `LINE_HEIGHT`).
#[must_use]
pub fn line_height() -> f32 {
    *LINE_HEIGHT_OVERRIDE
        .get_or_init(|| jetfish_protocol::Font::DEFAULT.line_height)
}

#[cfg(test)]
#[path = "../tests/tuning/mod.rs"]
mod tests;
