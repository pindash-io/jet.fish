// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! DECCARA / DECRARA rectangle attribute ops (DEC STD 070). [`rect_attr_mask`]
//! normalizes the SGR params; [`apply_rect_flags`] applies them to span flags.

use vtty::SgrFlags;

/// The four DECCARA/DECRARA attributes (bold, underline, blink, reverse):
/// the same bits as [`SgrFlags`].
const ATTR_ALL: SgrFlags = SgrFlags::from_bits_retain(
    SgrFlags::BOLD.bits()
        | SgrFlags::UNDERLINE.bits()
        | SgrFlags::BLINK.bits()
        | SgrFlags::INVERSE.bits(),
);

/// DECCARA/DECRARA: normalize the SGR params into `(mask, value)`, `None` for
/// a no-op.
#[must_use]
pub const fn rect_attr_mask(
    attrs: &[i32],
    set: bool,
) -> Option<(SgrFlags, SgrFlags)> {
    if attrs.is_empty() {
        // DECCARA: clear all four; DECRARA: no-op.
        return if set { Some((ATTR_ALL, SgrFlags::DEFAULT)) } else { None };
    }
    let mut mask = SgrFlags::DEFAULT;
    let mut value = SgrFlags::DEFAULT;
    let mut any = false;
    let mut i = 0;
    while i < attrs.len() {
        let a = attrs[i];
        i += 1;
        if a == 0 {
            if set {
                // SGR 0 clears all four.
                mask = ATTR_ALL;
                value = SgrFlags::DEFAULT;
            } else {
                // DECRARA 0 toggles all four.
                mask = mask.union(ATTR_ALL);
            }
            any = true;
            continue;
        }
        let (flag, on) = match (set, a) {
            (_, 1) => (SgrFlags::BOLD, true),
            (_, 4) => (SgrFlags::UNDERLINE, true),
            (_, 5) => (SgrFlags::BLINK, true),
            (_, 7) => (SgrFlags::INVERSE, true),
            (true, 22) => (SgrFlags::BOLD, false),
            (true, 24) => (SgrFlags::UNDERLINE, false),
            (true, 25) => (SgrFlags::BLINK, false),
            (true, 27) => (SgrFlags::INVERSE, false),
            // DECRARA ignores resets (vte: `DECRARA 3;23` == `DECRARA 3`).
            _ => continue,
        };
        mask = mask.union(flag);
        if set {
            if on {
                value = value.union(flag);
            } else {
                value = value.difference(flag);
            }
        }
        any = true;
    }
    if any {
        return Some((mask, value));
    }
    None
}

/// DECCARA: set the masked flags to `value`; DECRARA: toggle them.
pub const fn apply_rect_flags(
    flags: SgrFlags,
    mask: SgrFlags,
    value: SgrFlags,
    set: bool,
) -> SgrFlags {
    if set {
        flags.difference(mask).union(value.intersection(mask))
    } else {
        flags.symmetric_difference(mask)
    }
}

#[cfg(test)]
#[path = "../tests/deccara/mod.rs"]
mod tests;
