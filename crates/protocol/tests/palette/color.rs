// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

/// Lock the standard xterm extension to its known cube/grayscale values
/// (independent of `indexed_rgb`), so the two can never silently drift.
#[test]
fn extended_palette_is_standard_xterm() {
    let ext = standard_xterm_extended();
    // Cube corners: index 16 = (0,0,0), 21 = (0,0,5), 196 = (5,0,0).
    assert_eq!(ext[0], 0x000000ff); // 16 → black
    assert_eq!(ext[5], 0x0000ffff); // 21 → blue
    assert_eq!(ext[180], 0xff0000ff); // 196 → red
    assert_eq!(ext[215], 0xffffffff); // 231 → white
    // Grayscale ramp: index 232 = 0x08, 255 = 0xee.
    assert_eq!(ext[216], 0x080808ff); // 232
    assert_eq!(ext[239], 0xeeeeeeff); // 255
    // A mid-cube value: index 100 = (2,2,0) → (135, 135, 0).
    assert_eq!(ext[100 - 16], 0x878700ff); // 100
    // And the whole extension is the indexed_rgb table (the dedup).
    for (i, &v) in ext.iter().enumerate() {
        assert_eq!(v, indexed_rgb((i + 16) as u8));
    }
}
