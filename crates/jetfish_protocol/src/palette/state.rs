// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Dynamic terminal color palette: every color lives here, addressed by a
//! [`crate::ColorId`]. RGB truecolors are interned (deduplicated) on
//! insertion.

use crate::{ColorId, DEFAULT_BG_ID, DEFAULT_FG_ID};

use crate::palette::base16::{DEFAULT_BG, DEFAULT_FG};

/// Direct-mapped intern table size (power of two). Dedup is best-effort: a
/// collision evicts; a miss just re-interns.
const INTERN_SLOTS: usize = 8192;
const INTERN_MASK: usize = INTERN_SLOTS - 1;
/// Sentinel id meaning "empty slot". `u16::MAX` is never a valid id —
/// `entries` stops growing one short of it (`len() >= u16::MAX`).
const EMPTY_ID: u16 = u16::MAX;

/// Themed default colors (OSC 10/11), `None` = engine default.
#[derive(Debug, Default, Clone, Copy)]
pub struct DefaultColors {
    /// Themed default foreground (OSC 10).
    pub fg: Option<u32>,
    /// Themed default background (OSC 11).
    pub bg: Option<u32>,
}

/// Direct-mapped RGB → id table (boxed: 8192 `u16` slots are 16 KB).
#[derive(Debug, Clone)]
struct InternTable(Box<[u16; INTERN_SLOTS]>);

impl InternTable {
    #[inline]
    fn new() -> Self {
        Self(Box::new([EMPTY_ID; INTERN_SLOTS]))
    }

    #[inline]
    fn clear(&mut self) {
        *self.0 = [EMPTY_ID; INTERN_SLOTS];
    }
}

impl core::ops::Index<u32> for InternTable {
    type Output = u16;
    #[inline]
    fn index(&self, rgb: u32) -> &u16 {
        &self.0[rgb as usize & INTERN_MASK]
    }
}

impl core::ops::IndexMut<u32> for InternTable {
    #[inline]
    fn index_mut(&mut self, rgb: u32) -> &mut u16 {
        &mut self.0[rgb as usize & INTERN_MASK]
    }
}

/// The process-wide dynamic palette: 0..=255 xterm colors, 256/257 default
/// fg/bg, 258+ interned RGB truecolors.
#[derive(Debug, Clone)]
pub struct Palette {
    /// id → packed 0xRRGGBB.
    entries: Vec<u32>,
    /// Direct-mapped RGB → id (best-effort dedup): `Index`/`IndexMut` by RGB
    /// do the slot masking. No hash, no growth, no rehash.
    intern: InternTable,
    /// Themed default colors (OSC 10/11), `None` = engine default.
    default_colors: DefaultColors,
    /// 256-bit set: which palette indices the user overrode via OSC 4.
    /// Preserved across [`Palette::regenerate_from_theme`] and resets.
    mask: [u64; 4],
}

impl Palette {
    /// A fresh palette: 256 xterm colors + unset default fg/bg.
    ///
    /// The extended palette (16..256) is the standard xterm cube. Call
    /// [`Palette::regenerate_from_theme`] to generate it from the base16
    /// theme instead.
    pub fn new() -> Self {
        let mut entries = Vec::with_capacity(258);
        entries.extend_from_slice(&crate::palette::base16::base16_palette());
        entries.extend_from_slice(
            &crate::palette::color::standard_xterm_extended(),
        );
        entries.push(0); // 256 = default fg (unset)
        entries.push(0); // 257 = default bg (unset)
        Self {
            entries,
            intern: InternTable::new(),
            default_colors: DefaultColors::default(),
            mask: [0; 4],
        }
    }

    /// Regenerate the extended palette (16..256) from the current base16 +
    /// fg/bg using the color256 algorithm. Opt-in; the default is the
    /// standard xterm cube.
    ///
    /// Entries the user overrode via OSC 4 (tracked in the mask) are
    /// preserved; only unmasked extended entries are regenerated.
    pub fn regenerate_from_theme(&mut self, harmonious: bool) {
        let base16: [u32; 16] = self.entries[..16].try_into().unwrap();
        let bg = self.default_colors.bg.unwrap_or(DEFAULT_BG);
        let fg = self.default_colors.fg.unwrap_or(DEFAULT_FG);
        let generated = crate::palette::color::generate_256_palette(
            &base16, bg, fg, harmonious,
        );
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

    /// Intern an RGB truecolor, returning its id (deduplicated while the
    /// color is still in the direct-mapped table). Collisions evict; dedup
    /// is memory-only, so an evicted color just gets a fresh id.
    #[inline]
    pub fn insert_rgb(&mut self, rgb: u32) -> ColorId {
        let rgb = rgb & 0x00FF_FFFF;
        let existing = self.intern[rgb];
        // A hit needs the slot's id to still resolve to this RGB. `entries`
        // is grow-only (reset clears `intern`), so a recorded id is stable.
        if existing != EMPTY_ID && self.entries[existing as usize] == rgb {
            return ColorId(existing);
        }
        // `ColorId` is u16: at the space limit, stop inserting (a pathological
        // stream of distinct truecolors) rather than wrap an id.
        if self.entries.len() >= u16::MAX as usize {
            return DEFAULT_FG_ID;
        }
        let id = ColorId::from(self.entries.len());
        self.entries.push(rgb);
        self.intern[rgb] = id.0;
        id
    }

    /// Resolve an id to packed 0xRRGGBB. The default ids fall back to the
    /// themed defaults, then the engine defaults.
    #[inline]
    pub fn resolve(&self, id: ColorId, engine_fg: u32, engine_bg: u32) -> u32 {
        match id {
            DEFAULT_FG_ID => self.default_colors.fg.unwrap_or(engine_fg),
            DEFAULT_BG_ID => self.default_colors.bg.unwrap_or(engine_bg),
            _ => self.entries.get(id.0 as usize).copied().unwrap_or(0),
        }
    }

    /// The raw entry for an id (defaults resolve to `None` until themed).
    #[inline]
    pub fn get(&self, id: ColorId) -> Option<u32> {
        self.entries.get(id.0 as usize).copied()
    }

    /// The raw entries (id → packed 0xRRGGBB).
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

    /// Override an xterm palette entry (OSC 4). Out-of-range is ignored.
    /// The override is recorded in the mask so regeneration preserves it.
    pub fn set_entry(&mut self, idx: usize, rgb: u32) {
        if idx < 256 {
            self.entries[idx] = rgb;
            self.mask_set(idx);
        }
    }

    /// Reset one xterm palette entry to the theme default (OSC 104;N).
    pub fn reset_entry(&mut self, idx: usize) {
        if idx >= 256 {
            return;
        }
        if idx < 16 {
            self.entries[idx] = crate::palette::base16::base16_palette()[idx];
        } else {
            self.entries[idx] =
                crate::palette::color::standard_xterm_extended()[idx - 16];
        }
        self.mask_unset(idx);
    }

    /// Reset all xterm entries to theme defaults (OSC 104).
    ///
    /// The themed default fg/bg (OSC 10/11) are NOT touched — those are
    /// reset separately by OSC 110/111 via [`Palette::set_default_fg`] /
    /// [`Palette::set_default_bg`].
    pub fn reset(&mut self) {
        self.entries.truncate(258);
        self.entries[..16]
            .copy_from_slice(&crate::palette::base16::base16_palette());
        self.entries[16..256]
            .copy_from_slice(&crate::palette::color::standard_xterm_extended());
        self.entries[256] = 0;
        self.entries[257] = 0;
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
mod tests {
    use super::*;

    #[test]
    fn default_is_standard_xterm_extended() {
        let p = Palette::new();
        assert_eq!(
            &p.entries[0..16],
            &crate::palette::base16::base16_palette()[..]
        );
        assert_eq!(
            &p.entries[16..256],
            &crate::palette::color::standard_xterm_extended()[..]
        );
    }

    #[test]
    fn set_entry_overrides_without_regenerating() {
        let mut p = Palette::new();
        p.set_entry(1, 0x00FF00);
        assert_eq!(p.entries[1], 0x00FF00);
        // The standard extended palette is untouched by a base16 override.
        assert_eq!(
            p.entries[16],
            crate::palette::color::standard_xterm_extended()[0]
        );
    }

    #[test]
    fn reset_entry_restores_theme_base16() {
        let mut p = Palette::new();
        p.set_entry(1, 0x00FF00);
        p.reset_entry(1);
        assert_eq!(p.entries[1], crate::palette::base16::base16_palette()[1]);
    }

    #[test]
    fn reset_restores_standard_palette() {
        let mut p = Palette::new();
        let original = p.entries.clone();
        p.set_entry(1, 0x00FF00);
        p.set_default_bg(Some(0x000000));
        p.reset();
        assert_eq!(p.entries, original);
    }

    #[test]
    fn reset_preserves_default_fg_bg() {
        let mut p = Palette::new();
        p.set_default_fg(Some(0x123456));
        p.set_default_bg(Some(0x654321));
        p.set_entry(1, 0x00FF00);
        p.reset();
        // OSC 104 resets palette entries, but not the OSC 10/11 defaults.
        assert_eq!(p.entries[1], crate::palette::base16::base16_palette()[1]);
        assert_eq!(p.default_fg(), Some(0x123456));
        assert_eq!(p.default_bg(), Some(0x654321));
        // OSC 110/111 clear them.
        p.set_default_fg(None);
        p.set_default_bg(None);
        assert_eq!(p.default_fg(), None);
        assert_eq!(p.default_bg(), None);
    }

    #[test]
    fn regenerate_from_theme_uses_color256() {
        let mut p = Palette::new();
        let standard_corner = p.entries[16]; // standard cube (0,0,0) = black
        p.regenerate_from_theme(false);
        // color256 anchors the dark corner to the bg (#282C34), not black.
        assert_ne!(p.entries[16], standard_corner);
        assert_eq!(
            &p.entries[0..16],
            &crate::palette::base16::base16_palette()[..]
        );
    }

    #[test]
    fn osc4_tracks_user_overrides() {
        let mut p = Palette::new();
        assert!(!p.mask_get(1));
        p.set_entry(1, 0x00FF00);
        assert!(p.mask_get(1));
        p.reset_entry(1);
        assert!(!p.mask_get(1));
        p.set_entry(200, 0x123456);
        assert!(p.mask_get(200));
        p.reset();
        assert!(!p.mask_get(200));
    }

    #[test]
    fn regenerate_preserves_user_set_extended_entries() {
        let mut p = Palette::new();
        // User sets an extended entry via OSC 4.
        p.set_entry(100, 0x123456);
        let standard_corner = p.entries[16];
        p.regenerate_from_theme(false);
        // The masked (user-set) entry survives regeneration.
        assert_eq!(p.entries[100], 0x123456);
        // An unmasked entry is regenerated.
        assert_ne!(p.entries[16], standard_corner);
    }

    #[test]
    fn insert_rgb_dedups_repeated_colors() {
        let mut p = Palette::new();
        let a = p.insert_rgb(0x112233);
        let b = p.insert_rgb(0x112233);
        assert_eq!(a, b, "repeated RGB must reuse its id");
        assert_ne!(
            a,
            p.insert_rgb(0x445566),
            "distinct RGBs get distinct ids"
        );
        // A color whose low 13 bits collide with `a` evicts it (best-effort
        // dedup); re-inserting `a`'s RGB then yields a fresh id.
        let colliding = p.insert_rgb(0x112233 + INTERN_SLOTS as u32);
        assert_ne!(a, colliding);
        assert_ne!(a, p.insert_rgb(0x112233));
    }

    #[test]
    fn reset_clears_interned_truecolors() {
        let mut p = Palette::new();
        let id = p.insert_rgb(0x112233);
        assert_eq!(p.get(id), Some(0x112233));
        p.reset();
        // After reset the truecolor entries are gone; re-inserting starts
        // over from the first interned id.
        assert!(p.is_empty());
        assert_eq!(p.insert_rgb(0x112233), ColorId(258));
    }
}
