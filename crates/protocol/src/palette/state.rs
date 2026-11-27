// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Dynamic terminal color palette, addressed by [`vtty::ColorId`]; RGB
//! truecolors are interned on insertion.

use smallvec::SmallVec;

use vtty::ColorId;
use vtty::{DEFAULT_BG_ID, DEFAULT_FG_ID};

use crate::HashMap;
use crate::palette::base16::base16_palette;
use crate::palette::base16::{DEFAULT_BG, DEFAULT_FG};
use crate::palette::color::{generate_256_palette, standard_xterm_extended};

/// Themed default colors (OSC 10/11), `None` = engine default.
#[derive(Debug, Default, Clone, Copy)]
pub struct DefaultColors {
    /// Themed default foreground (OSC 10).
    pub fg: Option<u32>,
    /// Themed default background (OSC 11).
    pub bg: Option<u32>,
}

/// The process-wide dynamic palette: 0..=255 xterm colors, 256/257 default
/// fg/bg, 258+ interned RGB truecolors.
#[derive(Debug, Clone)]
pub struct Palette {
    /// id → packed 0xRRGGBBAA (256 xterm + 2 defaults inline, interned RGBs
    /// spill to the heap).
    entries: SmallVec<u32, 258>,
    /// RGB → id (exact, collision-free).
    intern: HashMap<u32, u16>,
    /// Themed default colors (OSC 10/11), `None` = engine default.
    default_colors: DefaultColors,
    /// Which indices the user overrode via OSC 4.
    mask: [u64; 4],
}

impl Palette {
    /// A fresh palette: 256 xterm colors + unset defaults (the standard
    /// xterm cube).
    #[must_use]
    pub fn new() -> Self {
        let mut entries = SmallVec::new();
        entries.extend_from_slice_copy(&base16_palette());
        entries.extend_from_slice_copy(&standard_xterm_extended());
        // 256 = default fg (unset), 257 = default bg (unset)
        entries.extend_from_slice_copy(&[0; 2]);

        Self {
            entries,
            intern: HashMap::default(),
            default_colors: DefaultColors::default(),
            mask: [0; 4],
        }
    }

    /// Regenerate the extended palette (16..256) from base16 + fg/bg; OSC 4
    /// overrides (the mask) are preserved.
    pub fn regenerate_from_theme(&mut self, harmonious: bool) {
        let base16: [u32; 16] = self.entries[..16].try_into().unwrap();
        let bg = self.default_colors.bg.unwrap_or(DEFAULT_BG);
        let fg = self.default_colors.fg.unwrap_or(DEFAULT_FG);
        let generated = generate_256_palette(&base16, bg, fg, harmonious);
        for (idx, &g) in generated.iter().enumerate().skip(16) {
            if !self.mask_get(idx) {
                self.entries[idx] = g;
            }
        }
    }

    #[inline]
    const fn mask_get(&self, idx: usize) -> bool {
        (self.mask[idx >> 6] >> (idx & 63)) & 1 == 1
    }

    #[inline]
    const fn mask_set(&mut self, idx: usize) {
        self.mask[idx >> 6] |= 1_u64 << (idx & 63);
    }

    #[inline]
    const fn mask_unset(&mut self, idx: usize) {
        self.mask[idx >> 6] &= !(1_u64 << (idx & 63));
    }

    /// Intern an RGB truecolor, returning its deduplicated id (collisions
    /// evict).
    #[must_use]
    #[inline]
    pub fn insert_rgb(&mut self, rgb: u32) -> ColorId {
        if let Some(&id) = self.intern.get(&rgb) {
            return ColorId(id);
        }
        // `ColorId` is u16: at the space limit, stop inserting (a pathological
        // stream of distinct truecolors) rather than wrap an id.
        if self.entries.len() >= u16::MAX as usize {
            return DEFAULT_FG_ID;
        }
        let id = ColorId::from(self.entries.len());
        self.entries.push(rgb);
        self.intern.insert(rgb, id.0);
        id
    }

    /// Resolve an id to packed 0xRRGGBBAA. The default ids fall back to the
    /// themed defaults, then the engine defaults.
    #[must_use]
    #[inline]
    pub fn resolve(&self, id: ColorId, engine_fg: u32, engine_bg: u32) -> u32 {
        match id {
            DEFAULT_FG_ID => self.default_colors.fg.unwrap_or(engine_fg),
            DEFAULT_BG_ID => self.default_colors.bg.unwrap_or(engine_bg),
            ColorId(id) => self.entries.get(id as usize).copied().unwrap_or(0),
        }
    }

    /// The raw entry for an id (defaults resolve to `None` until themed).
    #[must_use]
    #[inline]
    pub fn get(&self, id: ColorId) -> Option<u32> {
        self.entries.get(id.0 as usize).copied()
    }

    /// The entry's 24-bit RGB, alpha dropped (0 when unset). The kitty `U=1`
    /// placeholder packs an image/placement id into this value.
    #[must_use]
    #[inline]
    pub fn rgb(&self, id: ColorId) -> u32 {
        self.get(id).map_or(0, |c| c >> 8)
    }

    /// The raw entries (id → packed 0xRRGGBBAA).
    #[inline]
    pub fn entries(&self) -> &[u32] {
        &self.entries
    }

    /// The themed default foreground (OSC 10), if set.
    #[inline]
    pub const fn default_fg(&self) -> Option<u32> {
        self.default_colors.fg
    }

    /// The themed default background (OSC 11), if set.
    #[inline]
    pub const fn default_bg(&self) -> Option<u32> {
        self.default_colors.bg
    }

    /// Replace the 16 ANSI colors wholesale (the theme's base16). Unlike OSC 4
    /// these are theme defaults, so the user-override mask stays untouched.
    pub fn set_ansi(&mut self, ansi: &[u32; 16]) {
        self.entries[..16].copy_from_slice(ansi);
    }

    /// Override an xterm palette entry (OSC 4). Out-of-range is ignored.
    /// The override is recorded in the mask so regeneration preserves it.
    pub fn set_entry(&mut self, idx: usize, rgb: u32) {
        if idx >= 256 {
            return;
        }
        self.entries[idx] = rgb;
        self.mask_set(idx);
    }

    /// Reset one xterm palette entry to the theme default (OSC 104;N).
    pub fn reset_entry(&mut self, idx: usize) {
        if idx >= 256 {
            return;
        }
        if idx < 16 {
            self.entries[idx] = base16_palette()[idx];
        } else {
            self.entries[idx] = standard_xterm_extended()[idx - 16];
        }
        self.mask_unset(idx);
    }

    /// Reset all xterm entries to theme defaults (OSC 104); the OSC 10/11
    /// defaults are untouched.
    pub fn reset(&mut self) {
        self.entries.truncate(258);
        self.entries[..16].copy_from_slice(&base16_palette());
        self.entries[16..256].copy_from_slice(&standard_xterm_extended());
        self.entries[256..258].copy_from_slice(&[0; 2]);
        self.intern.clear();
        self.mask = [0; 4];
    }

    /// Set the themed default foreground (OSC 10).
    pub const fn set_default_fg(&mut self, rgb: Option<u32>) {
        self.default_colors.fg = rgb;
    }

    /// Set the themed default background (OSC 11).
    pub const fn set_default_bg(&mut self, rgb: Option<u32>) {
        self.default_colors.bg = rgb;
    }

    /// Number of entries (256 + 2 defaults + interned RGBs).
    pub const fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether only the 256 xterm colors + defaults are present.
    pub const fn is_empty(&self) -> bool {
        self.entries.len() <= 258
    }
}

impl Default for Palette {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "../../tests/palette/state.rs"]
mod tests;
