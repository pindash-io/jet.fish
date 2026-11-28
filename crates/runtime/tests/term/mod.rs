// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::utils::paste_bytes;

#[test]
fn bracketed_paste_wraps_multiline_text() {
    // The core fix: multi-line pastes stay inside the ESC[200~…201~ frame
    // so the child (pi/nvim) keeps them in the input buffer.
    assert_eq!(
        paste_bytes("line1\nline2", true),
        b"\x1b[200~line1\nline2\x1b[201~"
    );
}

#[test]
fn bracketed_paste_empty() {
    assert_eq!(paste_bytes("", true), b"\x1b[200~\x1b[201~");
}

#[test]
fn unbracketed_paste_keeps_text() {
    assert_eq!(paste_bytes("hello", false), b"hello");
}

#[test]
fn unbracketed_paste_converts_newline_to_cr() {
    // Without 2004, each newline submits.
    assert_eq!(paste_bytes("a\nb", false), b"a\rb");
    // Windows CRLF becomes CR CR.
    assert_eq!(paste_bytes("a\r\nb", false), b"a\r\rb");
}

#[test]
fn paste_strips_unsafe_control_bytes() {
    // ESC and other control bytes become spaces.
    assert_eq!(paste_bytes("a\x1bb", true), b"\x1b[200~a b\x1b[201~");
    assert_eq!(paste_bytes("a\x00b\x7fc", false), b"a b c");
}
