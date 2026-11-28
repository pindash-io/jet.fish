// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Small runtime helpers (paste encoding, engine ids).

/// Build the PTY paste bytes: bracketed wraps in `ESC[200~…201~`, unbracketed
/// maps `\n` to `\r`, and unsafe control bytes become spaces.
#[must_use]
pub fn paste_bytes(text: &str, bracketed: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() + 12);
    if bracketed {
        out.extend_from_slice(b"\x1b[200~");
    }
    for b in text.bytes() {
        match b {
            // Unsafe control bytes → space.
            0x00 | 0x04 | 0x05 | 0x08 | 0x0F | 0x11 | 0x12 | 0x13 | 0x15
            | 0x16 | 0x17 | 0x1A | 0x1B | 0x1C | 0x7F => out.push(b' '),
            // Unbracketed newline submits.
            b'\n' if !bracketed => out.push(b'\r'),
            b => out.push(b),
        }
    }
    if bracketed {
        out.extend_from_slice(b"\x1b[201~");
    }
    out
}

/// The next sequential terminal-engine id (`1, 2, 3, …`).
pub(crate) fn next_engine_id() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}
