// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Terminal theme/color types (the shared color state both screens read).

use std::sync::Arc;

use super::state::Palette;

/// Selection highlight colors (OSC 17 fg / OSC 19 bg). Generic over the color
/// representation; the screen shell uses `u32` (raw 0xRRGGBB).
#[derive(Debug, Default, Clone, Copy)]
pub struct Highlight<T = u32> {
    /// OSC 17 — selection foreground.
    pub fg: Option<T>,
    /// OSC 19 — selection background.
    pub bg: Option<T>,
}

/// The three terminal default colors: SGR 39 foreground, SGR 49 background,
/// and the OSC 12 cursor color. Generic over the color representation.
#[derive(Debug, Clone, Copy)]
pub struct Defaults<T> {
    /// SGR 39 — default foreground.
    pub fg: T,
    /// SGR 49 — default background.
    pub bg: T,
    /// OSC 12 — default cursor color (defaults to [`Self::fg`]).
    pub cursor: T,
}

impl Default for Defaults<u32> {
    /// Power-on defaults: the engine theme (cream on dark), so the raw
    /// [`Self::fg`]/[`Self::bg`] always match what the renderer draws.
    fn default() -> Self {
        let fg = super::base16::DEFAULT_FG;
        Self { fg, bg: super::base16::DEFAULT_BG, cursor: fg }
    }
}

/// The effective color scheme (`1` dark, `2` light): the host-reported value
/// when set, otherwise derived from the default background's relative
/// luminance.
pub fn color_scheme(detected: Option<u8>, default_bg: u32) -> u8 {
    detected.unwrap_or_else(|| {
        let [r, g, b] = crate::u32_to_rgb(default_bg);
        // sRGB relative luminance (Rec. 709 weights), scaled by 10000 —
        // integer arithmetic avoids the float division.
        let lum = 2126 * r as u32 + 7152 * g as u32 + 722 * b as u32;
        if lum < 1_280_000 { 1 } else { 2 }
    })
}

/// All shared color state (the terminal theme) in one place: the baked
/// palette, the three default colors, the selection highlight, and the
/// host-detected color scheme.
#[derive(Debug)]
pub struct Theme {
    /// The default palette (baked xterm-256 + defaults). Shared via `Arc`;
    /// each buffer keeps an optional themed override ([`Palette`]).
    pub palette: Arc<Palette>,
    /// The three default colors (fg/bg/cursor), raw 0xRRGGBB.
    pub defaults: Defaults<u32>,
    /// Selection highlight colors (OSC 17 fg / OSC 19 bg).
    pub highlight: Highlight<u32>,
    /// Host-reported color scheme (`Some(1)` dark, `Some(2)` light); `None`
    /// until reported, then derived from the default background luminance.
    pub detected_scheme: Option<u8>,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            palette: Arc::new(Palette::default()),
            defaults: Defaults::default(),
            highlight: Highlight::default(),
            detected_scheme: None,
        }
    }
}
