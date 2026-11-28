// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! App configuration: compiled constants + the `~/.config/jetfish/config.toml`
//! file format and loading. Every key is optional; absent keys keep the
//! compiled default.

use std::path::{Path, PathBuf};

use serde::Deserialize;

mod debounce;
mod font;
mod theme;
mod window;

pub use debounce::Debounce;
pub use font::Font;
pub use theme::ThemeConfig;
pub use window::Window;

/// Points → logical px (macOS 72 DPI = 1:1; elsewhere 96/72). The engine
/// multiplies by the window scale factor for physical px.
pub const POINT_TO_PX: f32 = cfg_select! {
    target_vendor = "apple" => 1.0,
    _ => 96.0 / 72.0,
};

/// The gamma-corrected blending factor passed to the glyph shader.
pub const GAMMA: [f32; 4] = [0.5, 0.5, 0.5, 0.3];

/// The user config: the raw `config.toml` file shape with compiled defaults.
#[derive(Deserialize, Default, Debug)]
#[serde(default)]
pub struct Config {
    /// Color theme (a named theme and/or inline colors).
    pub theme: ThemeConfig,
    /// Font settings.
    pub font: Font,
    /// Window appearance.
    pub window: Window,
    /// Input/layout debounce knobs.
    pub debounce: Debounce,
    /// SGR 1 bold maps the 8 normal ANSI fg colors to their bright half
    /// (the legacy xterm "bold-is-bright"). Off by default.
    #[serde(default)]
    pub bold_is_bright: bool,
}

impl Config {
    /// The config path: `$JETFISH_CONFIG`, else `~/.config/jetfish/config.toml`.
    #[must_use]
    pub fn path() -> PathBuf {
        if let Ok(p) = std::env::var("JETFISH_CONFIG") {
            return PathBuf::from(p);
        }
        config_base_dir().join("config.toml")
    }

    /// Load the config, tolerating a missing file and reporting a malformed
    /// one before falling back to defaults.
    #[must_use]
    pub fn load() -> Self {
        let path = Self::path();
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        Self::from_toml(&text, &path)
    }

    /// Parse the config TOML tolerantly; invalid sections/documents fall
    /// back to defaults.
    fn from_toml(text: &str, path: &Path) -> Self {
        let Ok(doc) = text.parse::<toml_edit::DocumentMut>() else {
            eprintln!(
                "jetfish: ignoring invalid config {path:?}: not valid TOML"
            );
            return Self::default();
        };
        Self {
            theme: doc
                .get("theme")
                .and_then(parse_section)
                .unwrap_or_default(),
            font: doc.get("font").and_then(parse_section).unwrap_or_default(),
            window: doc
                .get("window")
                .and_then(parse_section)
                .unwrap_or_default(),
            debounce: doc
                .get("debounce")
                .and_then(parse_section)
                .unwrap_or_default(),
            bold_is_bright: doc
                .get("bold_is_bright")
                .and_then(toml_edit::Item::as_bool)
                .unwrap_or_default(),
        }
    }

    /// Resolve the theme from the named file + inline keys.
    #[must_use]
    pub fn resolve_theme(&self) -> jetfish_protocol::Theme {
        self.theme.resolve(&theme_dirs())
    }
}

/// Deserialize a TOML table section into `T` (`None` on a type mismatch).
pub(crate) fn parse_section<T: serde::de::DeserializeOwned>(
    item: &toml_edit::Item,
) -> Option<T> {
    toml_edit::de::from_str(&item.to_string()).ok()
}

/// The theme directory: `~/.config/jetfish/themes`.
#[must_use]
pub fn user_themes_dir() -> PathBuf {
    config_base_dir().join("themes")
}

/// Theme search order: the user's directory (the only source).
#[must_use]
pub fn theme_dirs() -> Vec<PathBuf> {
    vec![user_themes_dir()]
}

/// The config base directory: `$JETFISH_CONFIG_DIR` or `~/.config/jetfish`.
fn config_base_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("JETFISH_CONFIG_DIR") {
        return PathBuf::from(dir);
    }
    std::env::home_dir().unwrap_or_default().join(".config").join("jetfish")
}

#[cfg(test)]
#[path = "../tests/lib/mod.rs"]
mod tests;
