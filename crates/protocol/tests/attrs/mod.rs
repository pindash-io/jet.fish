// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

#[test]
fn apply_rect_attr_sets_and_toggles() {
    let mut a = Attrs::DEFAULT;
    a = a.apply_rect_attr(SgrFlags::BOLD, SgrFlags::BOLD, true);
    assert!(a.bold());
    a = a.apply_rect_attr(SgrFlags::BOLD, SgrFlags::BOLD, false); // toggle off
    assert!(!a.bold());
    let all = SgrFlags::BOLD
        | SgrFlags::UNDERLINE
        | SgrFlags::BLINK
        | SgrFlags::INVERSE;
    a = a.apply_rect_attr(all, SgrFlags::empty(), true); // clear all
    assert!(!a.underline() && !a.blink() && !a.inverse());
}

/// SGR 6 (`BLINK_RAPID`) is parse-only and must not leak into the packed
/// underline-style bits (9..11).
#[test]
fn apply_flags_masks_parse_only_bits() {
    let a = Attrs::DEFAULT.apply_flags(SgrFlags::BLINK_RAPID, true);
    assert_eq!(a.underline_style(), UnderlineStyle::None);
    assert_eq!(a.flags(), SgrFlags::empty());
}

#[test]
fn span_flags_excludes_inverse() {
    let mut a = Attrs::DEFAULT;
    a = a.with_bold(true).with_italic(true).with_inverse(true);
    let flags = a.span_flags();
    assert!(flags.contains(SgrFlags::BOLD));
    assert!(flags.contains(SgrFlags::ITALIC));
    // INVERSE is excluded: the cell screen resolves it at RGB level.
    assert!(!flags.contains(SgrFlags::INVERSE));
}

#[test]
fn to_span_style_excludes_inverse_but_keeps_bold() {
    let a = Attrs::DEFAULT.with_bold(true).with_inverse(true);
    let style = a.to_span_style();
    assert!(style.flags.contains(SgrFlags::BOLD));
    assert!(!style.flags.contains(SgrFlags::INVERSE));
    // `to_style` keeps the FULL flags (INVERSE included).
    assert!(a.to_style().flags.contains(SgrFlags::INVERSE));
}
