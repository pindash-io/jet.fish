// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

#[test]
fn small_bursts_are_interactive_even_without_a_gap() {
    let d = Debounce::default();
    // Key echo + a few repeats: ≤ ECHO_BYTES, any gap.
    assert!(d.is_interactive(std::time::Duration::from_millis(5), 4));
    assert!(d.is_interactive(std::time::Duration::from_millis(16), 60));
}

#[test]
fn large_bursts_need_the_quiet_gap() {
    let d = Debounce::default();
    // `cat` bulk output: a large chunk with no quiet gap is streaming.
    assert!(!d.is_interactive(std::time::Duration::from_millis(1), 4096));
    // The same chunk after a quiet gap is a prompt/first line.
    assert!(d.is_interactive(std::time::Duration::from_millis(50), 4096));
}

#[test]
fn one_shot_large_bursts_are_streaming_but_settle_fast() {
    // A one-shot large burst (paste, TUI redraw, picker frame) is classified
    // streaming, but the 16ms settle repaints it one frame after it goes quiet
    // — the settle, not the classifier, is what makes paste/prompt feel fast.
    let d = Debounce::default();
    assert!(!d.is_interactive(std::time::Duration::from_millis(1), 1024));
    assert!(!d.is_interactive(std::time::Duration::from_millis(10), 4096));
}
