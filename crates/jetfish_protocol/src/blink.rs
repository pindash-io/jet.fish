// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! SGR 5 text-blink + cursor-blink phases (shared across renderer threads).

/// Global SGR 5 text-blink phase; while `false`, blinking spans render as
/// their background.
static BLINK_ON: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(true);

/// Global cursor-blink phase (DEC 12); while `false` a blinking cursor is
/// hidden.
static CURSOR_BLINK_ON: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(true);

/// Whether the current blink phase shows blinking text.
#[inline]
pub fn blink_on() -> bool {
    BLINK_ON.load(std::sync::atomic::Ordering::Relaxed)
}

/// Flip the text blink phase; returns the new phase.
#[inline]
pub fn toggle_blink() -> bool {
    BLINK_ON.fetch_xor(true, std::sync::atomic::Ordering::Relaxed) ^ true
}

/// Whether the current cursor-blink phase shows a blinking cursor.
#[inline]
pub fn cursor_blink_on() -> bool {
    CURSOR_BLINK_ON.load(std::sync::atomic::Ordering::Relaxed)
}

/// Flip the cursor-blink phase; returns the new phase.
#[inline]
pub fn toggle_cursor_blink() -> bool {
    CURSOR_BLINK_ON.fetch_xor(true, std::sync::atomic::Ordering::Relaxed)
        ^ true
}

/// Reset the cursor-blink phase to ON (the cursor is shown). Called on key
/// input so typing cancels the blink and the cursor stays visible.
#[inline]
pub fn reset_cursor_blink() {
    CURSOR_BLINK_ON.store(true, std::sync::atomic::Ordering::Relaxed);
}
