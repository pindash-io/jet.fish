// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

#[test]
fn default_is_standard_xterm_extended() {
    let p = Palette::new();
    assert_eq!(&p.entries[0..16], &base16_palette()[..]);
    assert_eq!(&p.entries[16..256], &standard_xterm_extended()[..]);
}

#[test]
fn set_entry_overrides_without_regenerating() {
    let mut p = Palette::new();
    p.set_entry(1, 0x00FF00);
    assert_eq!(p.entries[1], 0x00FF00);
    // The standard extended palette is untouched by a base16 override.
    assert_eq!(p.entries[16], standard_xterm_extended()[0]);
}

#[test]
fn reset_entry_restores_theme_base16() {
    let mut p = Palette::new();
    p.set_entry(1, 0x00FF00);
    p.reset_entry(1);
    assert_eq!(p.entries[1], base16_palette()[1]);
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
    assert_eq!(p.entries[1], base16_palette()[1]);
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
    assert_eq!(&p.entries[0..16], &base16_palette()[..]);
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
    assert_ne!(a, p.insert_rgb(0x445566), "distinct RGBs get distinct ids");
    // The exact map is collision-free: a low-bit-colliding RGB is a distinct
    // entry, so re-inserting `a` still hits.
    let colliding = p.insert_rgb(0x112233 + 8192);
    assert_ne!(a, colliding);
    assert_eq!(a, p.insert_rgb(0x112233));
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
