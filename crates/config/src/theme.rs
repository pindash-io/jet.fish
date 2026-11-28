// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Color theme: the [`ThemeConfig`] section and its resolved form.

use std::path::PathBuf;

use compact_str::CompactString;
use serde::Deserialize;

use vtty::hex_to_rgba_u32 as parse_hex;

use jetfish_protocol::Theme;

/// A color theme: a named theme file and/or inline colors. Generic over the
/// color representation: `CompactString` (config hex) or `u32` (resolved).
#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct ThemeConfig<T = CompactString> {
    /// A bundled theme name (`name = "Flexoki Light"`), optional.
    pub name: Option<CompactString>,
    /// Default foreground, hex `#RRGGBB`.
    pub foreground: Option<T>,
    /// Default background, hex `#RRGGBB`.
    pub background: Option<T>,
    /// Cursor color, hex `#RRGGBB`.
    pub cursor: Option<T>,
    /// Cursor text color, hex `#RRGGBB` (accepted for theme compat; the
    /// block cursor inverts with the default background instead).
    pub cursor_text: Option<T>,
    /// Selection background, hex `#RRGGBB`.
    pub selection: Option<T>,
    /// Selection foreground (text), hex `#RRGGBB`.
    pub selection_text: Option<T>,
    /// The 16 palette colors, hex `#RRGGBB` (index 0..15).
    pub palette: Vec<T>,
}

impl<T> Default for ThemeConfig<T> {
    fn default() -> Self {
        Self {
            name: None,
            foreground: None,
            background: None,
            cursor: None,
            cursor_text: None,
            selection: None,
            selection_text: None,
            palette: Vec::new(),
        }
    }
}

impl ThemeConfig<CompactString> {
    /// The named theme file (if `name`) overlaid with the inline keys.
    #[must_use]
    pub fn resolve(&self, themes_dirs: &[PathBuf]) -> Theme {
        let mut resolved = ThemeConfig::<u32>::default();
        if let Some(name) = &self.name {
            let file = format!("{name}.toml");
            for dir in themes_dirs {
                let path = dir.join(&file);
                if let Ok(text) = std::fs::read_to_string(&path)
                    && let Ok(named) = toml_edit::de::from_str::<Self>(&text)
                {
                    named.merge_into(&mut resolved);
                    break;
                }
            }
        }
        self.merge_into(&mut resolved);

        let mut ansi = jetfish_protocol::base16_palette();
        for (i, &v) in resolved.palette.iter().take(16).enumerate() {
            ansi[i] = v;
        }
        Theme::themed(
            &ansi,
            resolved.foreground,
            resolved.background,
            resolved.cursor,
            resolved.selection,
            resolved.selection_text,
        )
    }

    /// Overlay the set keys over `resolved`.
    fn merge_into(&self, resolved: &mut ThemeConfig<u32>) {
        for (i, v) in self
            .palette
            .iter()
            .take(16)
            .map(CompactString::as_str)
            .flat_map(parse_hex)
            .enumerate()
        {
            if i < resolved.palette.len() {
                resolved.palette[i] = v;
            } else {
                resolved.palette.push(v);
            }
        }
        if let Some(v) = self.foreground.as_deref().and_then(parse_hex) {
            resolved.foreground = Some(v);
        }
        if let Some(v) = self.background.as_deref().and_then(parse_hex) {
            resolved.background = Some(v);
        }
        if let Some(v) = self.cursor.as_deref().and_then(parse_hex) {
            resolved.cursor = Some(v);
        }
        if let Some(v) = self.selection.as_deref().and_then(parse_hex) {
            resolved.selection = Some(v);
        }
        if let Some(v) = self.selection_text.as_deref().and_then(parse_hex) {
            resolved.selection_text = Some(v);
        }
    }
}

#[cfg(test)]
#[path = "../tests/theme/mod.rs"]
mod tests;
