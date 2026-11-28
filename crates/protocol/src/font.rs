// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Resolved font metrics passed to shaping and rasterization.

use compact_str::CompactString;
use serde::Deserialize;

/// The resolved font parameters the shaping/rasterization layers need.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default = "Font::default")]
pub struct Font {
    /// Base font size in logical pixels.
    pub size: f32,
    /// Line height relative to the font size.
    pub line_height: f32,
    /// Regular font weight (400).
    pub weight: f32,
    /// Bold font weight (700).
    pub bold_weight: f32,
    /// Physical-to-logical pixel scale (2.0 on `HiDPI`).
    pub scale_factor: f32,
    /// The primary font family name; `None` auto-discovers the OS monospace.
    pub family: Option<CompactString>,
    /// OpenType ligatures + contextual alternates.
    pub ligatures: bool,
    /// Text-default emoji-capable glyphs render as their mono outline instead
    /// of colour emoji.
    pub text_emoji_outline: bool,
}

impl Font {
    /// The compiled default font metrics.
    pub const DEFAULT: Self = Self {
        size: 14.0,
        line_height: 1.2,
        weight: 400.0,
        bold_weight: 700.0,
        scale_factor: 2.0,
        family: None,
        ligatures: true,
        text_emoji_outline: false,
    };
}

impl Default for Font {
    fn default() -> Self {
        Self::DEFAULT
    }
}
