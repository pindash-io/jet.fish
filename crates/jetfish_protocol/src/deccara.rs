// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! DECCARA / DECRARA rectangle attribute ops (DEC STD 070). [`rect_attr_mask`]
//! normalizes the SGR params; [`apply_rect_flags`] applies them to span flags.

use crate::SgrFlags;

/// The four DECCARA/DECRARA attributes (bold, underline, blink, reverse) —
/// the same bits as [`SgrFlags`].
const ATTR_ALL: SgrFlags = SgrFlags::from_bits_retain(
    SgrFlags::BOLD.bits()
        | SgrFlags::UNDERLINE.bits()
        | SgrFlags::BLINK.bits()
        | SgrFlags::INVERSE.bits(),
);

/// Normalize the SGR params after the four rectangle coordinates into a
/// `(mask, value)` pair. `set` selects DECCARA (set/clear) vs DECRARA
/// (toggle). Returns `None` for a no-op (empty DECRARA).
pub fn rect_attr_mask(
    attrs: &[i32],
    set: bool,
) -> Option<(SgrFlags, SgrFlags)> {
    if attrs.is_empty() {
        // DECCARA: clear all four; DECRARA: no-op.
        return if set { Some((ATTR_ALL, SgrFlags::empty())) } else { None };
    }
    let mut mask = SgrFlags::empty();
    let mut value = SgrFlags::empty();
    let mut any = false;
    for &a in attrs {
        if a == 0 {
            if set {
                // SGR 0 clears all four.
                mask = ATTR_ALL;
                value = SgrFlags::empty();
            } else {
                // DECRARA 0 toggles all four.
                mask |= ATTR_ALL;
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
        mask |= flag;
        if set {
            if on {
                value |= flag;
            } else {
                value &= !flag;
            }
        }
        any = true;
    }
    if !any { None } else { Some((mask, value)) }
}

/// Apply a DECCARA/DECRARA change to span-level [`SgrFlags`]: DECCARA sets the
/// masked flags to `value`, DECRARA toggles them.
pub fn apply_rect_flags(
    flags: &mut SgrFlags,
    mask: SgrFlags,
    value: SgrFlags,
    set: bool,
) {
    if set {
        *flags = (*flags - mask) | (value & mask);
    } else {
        *flags ^= mask;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_attr_mask_normalizes_set_and_toggle() {
        // DECCARA 1 (bold) → mask bold, value bold.
        assert_eq!(
            rect_attr_mask(&[1], true),
            Some((SgrFlags::BOLD, SgrFlags::BOLD))
        );
        // DECCARA 22 (not bold) → mask bold, value 0.
        assert_eq!(
            rect_attr_mask(&[22], true),
            Some((SgrFlags::BOLD, SgrFlags::empty()))
        );
        // DECCARA empty → clear all four.
        assert_eq!(
            rect_attr_mask(&[], true),
            Some((ATTR_ALL, SgrFlags::empty()))
        );
        // DECCARA 0 → clear all four.
        assert_eq!(
            rect_attr_mask(&[0], true),
            Some((ATTR_ALL, SgrFlags::empty()))
        );
        // DECRARA empty → no-op.
        assert_eq!(rect_attr_mask(&[], false), None);
        // DECRARA 0 → toggle all four.
        assert_eq!(
            rect_attr_mask(&[0], false),
            Some((ATTR_ALL, SgrFlags::empty()))
        );
        // DECRARA 1 → toggle bold only (resets ignored).
        assert_eq!(
            rect_attr_mask(&[1, 22], false),
            Some((SgrFlags::BOLD, SgrFlags::empty()))
        );
    }
}
