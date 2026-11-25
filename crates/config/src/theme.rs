// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Color theme: the [`Theme`] config section and its resolved form.

use std::path::PathBuf;

use compact_str::CompactString;
use serde::Deserialize;

/// A color theme: a named theme file and/or inline colors.
#[derive(Deserialize, Default, Debug, Clone)]
#[serde(default)]
pub struct Theme {
    /// A bundled theme name (`name = "Flexoki Light"`), optional.
    pub name: Option<CompactString>,
    /// Default foreground, hex `#RRGGBB`.
    pub foreground: Option<CompactString>,
    /// Default background, hex `#RRGGBB`.
    pub background: Option<CompactString>,
    /// Cursor color, hex `#RRGGBB`.
    pub cursor: Option<CompactString>,
    /// Cursor text color, hex `#RRGGBB` (accepted for theme compat; the
    /// block cursor inverts with the default background instead).
    pub cursor_text: Option<CompactString>,
    /// Selection background, hex `#RRGGBB`.
    pub selection: Option<CompactString>,
    /// Selection foreground (text), hex `#RRGGBB`.
    pub selection_text: Option<CompactString>,
    /// The 16 palette colors, hex `#RRGGBB` (index 0..15).
    pub palette: Vec<CompactString>,
}

/// A resolved theme: the 16 packed `0xRRGGBBAA` ANSI colors + default fg/bg.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedTheme {
    /// The 16 ANSI colors (index 0..15).
    pub ansi: [u32; 16],
    /// The default foreground (unset → engine default).
    pub foreground: Option<u32>,
    /// The default background (unset → engine default).
    pub background: Option<u32>,
    /// The cursor color (unset → foreground).
    pub cursor: Option<u32>,
    /// The selection background (unset → default).
    pub selection: Option<u32>,
    /// The selection foreground/text (unset → default).
    pub selection_text: Option<u32>,
}

impl Default for ResolvedTheme {
    fn default() -> Self {
        Self {
            ansi: jetfish_protocol::base16_palette(),
            foreground: None,
            background: None,
            cursor: None,
            selection: None,
            selection_text: None,
        }
    }
}

impl Theme {
    /// The named theme file (if `name`) overlaid with the inline keys.
    #[must_use]
    pub fn resolve(&self, themes_dirs: &[PathBuf]) -> ResolvedTheme {
        let mut resolved = ResolvedTheme::default();
        if let Some(name) = &self.name {
            let file = format!("{name}.toml");
            for dir in themes_dirs {
                let path = dir.join(&file);
                if let Ok(text) = std::fs::read_to_string(&path)
                    && let Ok(named) = toml_edit::de::from_str::<Self>(&text)
                {
                    merge_theme(&named, &mut resolved);
                    break;
                }
            }
        }
        merge_theme(self, &mut resolved);
        resolved
    }
}

/// Overlay `t`'s set keys over `resolved`.
fn merge_theme(t: &Theme, resolved: &mut ResolvedTheme) {
    for (i, hex) in t.palette.iter().take(16).enumerate() {
        if let Some(rgb) = parse_hex(hex) {
            resolved.ansi[i] = rgb;
        }
    }
    if let Some(v) = t.foreground.as_deref().and_then(parse_hex) {
        resolved.foreground = Some(v);
    }
    if let Some(v) = t.background.as_deref().and_then(parse_hex) {
        resolved.background = Some(v);
    }
    if let Some(v) = t.cursor.as_deref().and_then(parse_hex) {
        resolved.cursor = Some(v);
    }
    if let Some(v) = t.selection.as_deref().and_then(parse_hex) {
        resolved.selection = Some(v);
    }
    if let Some(v) = t.selection_text.as_deref().and_then(parse_hex) {
        resolved.selection_text = Some(v);
    }
}

/// Parse a `#RRGGBB` / `RRGGBB` hex string (or the 3-digit `#RGB` shorthand,
/// expanded to `#RRGGBB`) into packed `0xRRGGBBAA` (opaque).
pub(crate) fn parse_hex(s: &str) -> Option<u32> {
    let s = s.trim().trim_start_matches('#');
    let expanded: String = match s.len() {
        6 => s.to_owned(),
        3 => s.chars().flat_map(|c| [c, c]).collect(),
        _ => return None,
    };
    let v = u32::from_str_radix(&expanded, 16).ok()?;
    Some((v << 8) | 0xFF)
}

#[cfg(test)]
#[path = "../tests/theme/mod.rs"]
mod tests;
