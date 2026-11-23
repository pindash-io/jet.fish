// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Font settings: the [`Font`] config section.

use compact_str::CompactString;
use serde::Deserialize;

/// Font settings.
#[derive(Deserialize, Debug)]
#[serde(default = "Font::default")]
pub struct Font {
    /// Family name or a path to a `.ttf`/`.otf`; `None` auto-discovers the OS
    /// monospace.
    pub family: Option<CompactString>,
    /// Size in points.
    pub size: f32,
    /// Line height as a multiple of the font size (the cell-grid row height).
    pub line_height: f32,
    /// Regular font weight (400).
    pub weight: f32,
    /// Bold font weight (700).
    pub bold_weight: f32,
    /// Physical-to-logical pixel scale (2.0 on `HiDPI`).
    pub scale_factor: f32,
    /// OpenType ligatures + contextual alternates.
    pub ligatures: bool,
    /// Text-default emoji-capable glyphs render as their mono outline instead
    /// of colour emoji.
    pub text_emoji_outline: bool,
}

impl Default for Font {
    fn default() -> Self {
        let f = jetfish_protocol::Font::DEFAULT;
        Self {
            family: None,
            size: f.size,
            line_height: f.line_height,
            weight: f.weight,
            bold_weight: f.bold_weight,
            scale_factor: f.scale_factor,
            ligatures: true,
            text_emoji_outline: false,
        }
    }
}

impl Font {
    /// The resolved shaping/rasterization metrics.
    #[must_use]
    pub fn resolve(&self) -> jetfish_protocol::Font {
        jetfish_protocol::Font {
            size: self.size,
            line_height: self.line_height.clamp(1.0, 3.0),
            weight: self.weight,
            bold_weight: self.bold_weight,
            scale_factor: self.scale_factor,
            family: self.family.clone(),
        }
    }
}
