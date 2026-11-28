// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Terminal theme/color types.

use std::sync::Arc;

use vtty::u32_to_array;

use super::state::Palette;

/// Selection highlight colors (OSC 17 fg / OSC 19 bg). Generic over the color
/// representation; the screen shell uses `u32` (raw 0xRRGGBBAA).
#[derive(Debug, Default, Clone, Copy)]
pub struct Highlight<T = u32> {
    /// OSC 17: selection foreground.
    pub fg: Option<T>,
    /// OSC 19: selection background.
    pub bg: Option<T>,
}

/// The three terminal default colors: SGR 39 foreground, SGR 49 background,
/// and the OSC 12 cursor color. Generic over the color representation.
#[derive(Debug, Clone, Copy)]
pub struct Defaults<T> {
    /// SGR 39: default foreground.
    pub fg: T,
    /// SGR 49: default background.
    pub bg: T,
    /// OSC 12: default cursor color (defaults to [`Self::fg`]).
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

/// All shared color state in one place: the baked palette, the three defaults,
/// the selection highlight and the host-detected scheme.
#[derive(Clone, Debug)]
pub struct Theme {
    /// The default palette (baked xterm-256 + defaults). Shared via `Arc`;
    /// each buffer keeps an optional themed override ([`Palette`]).
    pub palette: Arc<Palette>,
    /// The three default colors (fg/bg/cursor), raw 0xRRGGBBAA.
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

impl Theme {
    /// A themed instance from the resolved 16 ANSI colors + defaults.
    #[must_use]
    pub fn themed(
        ansi: &[u32; 16],
        foreground: Option<u32>,
        background: Option<u32>,
        cursor: Option<u32>,
        selection: Option<u32>,
        selection_text: Option<u32>,
    ) -> Self {
        let mut theme = Self::default();
        let mut palette = theme.palette.as_ref().clone();
        palette.set_ansi(ansi);
        palette.set_default_fg(foreground);
        palette.set_default_bg(background);
        theme.palette = Arc::new(palette);
        if let Some(v) = foreground {
            theme.defaults.fg = v;
        }
        if let Some(v) = background {
            theme.defaults.bg = v;
        }
        if let Some(v) = cursor {
            theme.defaults.cursor = v;
        }
        theme.highlight.fg = selection_text;
        theme.highlight.bg = selection;
        theme
    }
}

/// The effective scheme (`1` dark, `2` light): host-reported when set, else
/// derived from the default background's relative luminance.
#[must_use]
pub fn color_scheme(detected: Option<u8>, default_bg: u32) -> u8 {
    detected.unwrap_or_else(|| {
        let [r, g, b] = u32_to_array(default_bg);
        // sRGB relative luminance (Rec. 709 weights) scaled by 10000: integer
        // arithmetic avoids the float division.
        let lum = 2126 * r as u32 + 7152 * g as u32 + 722 * b as u32;
        if lum < 1_280_000 { 1 } else { 2 }
    })
}
