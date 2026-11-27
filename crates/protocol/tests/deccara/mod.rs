// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

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
        Some((SgrFlags::BOLD, SgrFlags::DEFAULT))
    );
    // DECCARA empty → clear all four.
    assert_eq!(rect_attr_mask(&[], true), Some((ATTR_ALL, SgrFlags::DEFAULT)));
    // DECCARA 0 → clear all four.
    assert_eq!(
        rect_attr_mask(&[0], true),
        Some((ATTR_ALL, SgrFlags::DEFAULT))
    );
    // DECRARA empty → no-op.
    assert_eq!(rect_attr_mask(&[], false), None);
    // DECRARA 0 → toggle all four.
    assert_eq!(
        rect_attr_mask(&[0], false),
        Some((ATTR_ALL, SgrFlags::DEFAULT))
    );
    // DECRARA 1 → toggle bold only (resets ignored).
    assert_eq!(
        rect_attr_mask(&[1, 22], false),
        Some((SgrFlags::BOLD, SgrFlags::DEFAULT))
    );
}
