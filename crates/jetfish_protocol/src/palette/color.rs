// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! ANSI / xterm color helpers shared by the terminal engines.
//!
//! The wire-format primitives (RGB packing, OSC `rgb:` parsing, OSC 10/11/4
//! reply formatting) live in `vtty_core::color`, and the SGR color resolution
//! (`blend_faint` / `apply_sgr_effects`) lives in `jetfish_protocol::attrs`;
//! this module keeps the palette-side color math.

/// The standard xterm 256-color extension (indices 16..256): a 6×6×6 cube
/// with channel values 0, 95, 135, 175, 215, 255, followed by a 24-step
/// grayscale ramp 8, 18, …, 238. The per-index math is [`crate::indexed_rgb`];
/// this only materializes indices 16..=255 into a lookup array.
pub const fn standard_xterm_extended() -> [u32; 240] {
    let mut pal = [0_u32; 240];
    let mut i = 0;
    while i < 240 {
        pal[i] = crate::indexed_rgb((i + 16) as u8);
        i += 1;
    }
    pal
}

/// Generate the full 256-color palette from a base16 theme; `bg`/`fg` anchor
/// the cube's two achromatic corners.
pub fn generate_256_palette(
    base16: &[u32; 16],
    bg: u32,
    fg: u32,
    harmonious: bool,
) -> [u32; 256] {
    use super::cielab::{lab_to_rgb, lerp_lab, rgb_to_lab};

    let mut base8_lab = [
        rgb_to_lab(bg),
        rgb_to_lab(base16[1]),
        rgb_to_lab(base16[2]),
        rgb_to_lab(base16[3]),
        rgb_to_lab(base16[4]),
        rgb_to_lab(base16[5]),
        rgb_to_lab(base16[6]),
        rgb_to_lab(fg),
    ];

    // Light themes (foreground darker than background) need their achromatic
    // cube corners swapped so 16 → black and 231 → white.
    let is_light_theme = base8_lab[7][0] < base8_lab[0][0];
    if is_light_theme && !harmonious {
        base8_lab.swap(0, 7);
    }

    let mut palette = [0_u32; 256];
    palette[..16].copy_from_slice(base16);

    // 216-color cube: trilinear interpolation of the 8 corners.
    let mut idx = 16;
    for r in 0..6 {
        let tr = r as f32 / 5.0;
        let c0 = lerp_lab(tr, base8_lab[0], base8_lab[1]);
        let c1 = lerp_lab(tr, base8_lab[2], base8_lab[3]);
        let c2 = lerp_lab(tr, base8_lab[4], base8_lab[5]);
        let c3 = lerp_lab(tr, base8_lab[6], base8_lab[7]);
        for g in 0..6 {
            let tg = g as f32 / 5.0;
            let c4 = lerp_lab(tg, c0, c1);
            let c5 = lerp_lab(tg, c2, c3);
            for b in 0..6 {
                let c6 = lerp_lab(b as f32 / 5.0, c4, c5);
                palette[idx] = lab_to_rgb(c6);
                idx += 1;
            }
        }
    }

    // 24-step grayscale ramp between the two achromatic corners.
    for i in 0..24 {
        let t = (i as f32 + 1.0) / 25.0;
        palette[idx] = lab_to_rgb(lerp_lab(t, base8_lab[0], base8_lab[7]));
        idx += 1;
    }

    palette
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lock the standard xterm extension to its known cube/grayscale values
    /// (independent of `indexed_rgb`), so the two can never silently drift.
    #[test]
    fn extended_palette_is_standard_xterm() {
        let ext = standard_xterm_extended();
        // Cube corners: index 16 = (0,0,0), 21 = (0,0,5), 196 = (5,0,0).
        assert_eq!(ext[0], 0x000000); // 16 → black
        assert_eq!(ext[5], 0x0000ff); // 21 → blue
        assert_eq!(ext[180], 0xff0000); // 196 → red
        assert_eq!(ext[215], 0xffffff); // 231 → white
        // Grayscale ramp: index 232 = 0x08, 255 = 0xee.
        assert_eq!(ext[216], 0x080808); // 232
        assert_eq!(ext[239], 0xeeeeee); // 255
        // A mid-cube value: index 100 = (2,2,0) → (135, 135, 0).
        assert_eq!(ext[100 - 16], 0x878700); // 100
        // And the whole extension is the indexed_rgb table (the dedup).
        for (i, &v) in ext.iter().enumerate() {
            assert_eq!(v, crate::indexed_rgb((i + 16) as u8));
        }
    }
}
