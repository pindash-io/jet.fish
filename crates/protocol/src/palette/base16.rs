// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The base16 theme: default foreground/background and the 16 ANSI colors.

use vtty::rgba_to_u32;

/// The conventional default foreground (ANSI black, `0x100F0FFF`).
pub const DEFAULT_FG: u32 = 0x10_0F_0F_FF;

/// The conventional default background (ANSI white, `0xFFFCF0FF`); must stay in
/// sync with the GUI window clear color.
pub const DEFAULT_BG: u32 = 0xFF_FC_F0_FF;

/// The base16 theme colors, packed as `0xRRGGBBAA` (the baked 16-color ANSI
/// palette).
#[must_use]
pub const fn base16_palette() -> [u32; 16] {
    let palette = default_palette();
    let mut base = [0_u32; 16];
    let mut i = 0;
    while i < 16 {
        base[i] = rgba_to_u32(palette[i]);
        i += 1;
    }
    base
}

/// The baked 16-color ANSI palette (compile-time constant).
#[inline]
const fn default_palette() -> [[f32; 4]; 16] {
    const DEFAULT: [[u8; 3]; 16] = [
        [0x10, 0x0F, 0x0F],
        [0xAF, 0x30, 0x29],
        [0x66, 0x80, 0x0B],
        [0xAD, 0x83, 0x01],
        [0x20, 0x5E, 0xA6],
        [0xA0, 0x2F, 0x6F],
        [0x24, 0x83, 0x7B],
        [0xCE, 0xCD, 0xC4],
        [0x6F, 0x6E, 0x69],
        [0xD1, 0x4D, 0x41],
        [0x87, 0x9A, 0x39],
        [0xD0, 0xA2, 0x15],
        [0x43, 0x85, 0xBE],
        [0xCE, 0x5D, 0x97],
        [0x3A, 0xA9, 0x9F],
        [0xFF, 0xFC, 0xF0],
    ];
    let mut palette = [[0.0_f32; 4]; 16];
    let mut i = 0;
    while i < 16 {
        palette[i] = [
            DEFAULT[i][0] as f32 / 255.0,
            DEFAULT[i][1] as f32 / 255.0,
            DEFAULT[i][2] as f32 / 255.0,
            1.0,
        ];
        i += 1;
    }
    palette
}
