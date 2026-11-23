// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! App configuration: compiled constants + the `~/.config/jetfish/config.toml`
//! file format and loading. Every config key is optional; absent keys keep
//! the compiled default (or the `JETFISH_*` env override).

use std::path::{Path, PathBuf};

use serde::Deserialize;

mod debounce;
mod font;
mod theme;
mod window;

pub use debounce::Debounce;
pub use font::Font;
pub use theme::{ResolvedTheme, Theme};
pub use window::Window;

/// Points → logical px (macOS 72 DPI = 1:1; elsewhere 96/72). The engine
/// multiplies by the window scale factor for physical px.
pub const POINT_TO_PX: f32 =
    if cfg!(target_os = "macos") { 1.0 } else { 96.0 / 72.0 };

/// The default window background color (paper cream #FFFCF0).
pub const BACKGROUND_COLOR: [f32; 4] = [1.0, 0.9882, 0.9412, 1.0];

/// The gamma-corrected blending factor passed to the glyph shader.
pub const GAMMA: [f32; 4] = [0.5, 0.5, 0.5, 0.3];

/// The user config. `Option` fields mean "unset → default"; this is the raw
/// file shape, resolved by [`Config::resolve`] into an `Option`-free view.
#[derive(Deserialize, Default, Debug)]
#[serde(default)]
pub struct Config {
    /// Color theme (a named theme and/or inline colors).
    pub theme: Theme,
    /// Font family + size.
    pub font: Font,
    /// Window appearance.
    pub window: Window,
    /// Input/layout debounce knobs.
    pub debounce: Debounce,
    /// SGR 1 bold maps the 8 normal ANSI fg colors to their bright half
    /// (the legacy xterm "bold-is-bright"). Off by default (Ghostty's
    /// default).
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

    /// Parse the config TOML tolerantly. Unknown keys are ignored; a section
    /// whose value has the wrong type falls back to that section's default.
    /// A syntactically invalid document falls back to all defaults.
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

    /// Resolve against the compiled defaults (the env overrides are applied
    /// by the caller on top of this). Themes resolve from the user's
    /// `~/.config/jetfish/themes` first, then the bundled dir.
    #[must_use]
    pub fn resolve_theme(&self) -> ResolvedTheme {
        self.theme.resolve(&theme_dirs())
    }
}

/// Deserialize a TOML table section into `T`, returning `None` on a type
/// mismatch so the caller keeps the default for that section. Unknown keys
/// inside the section are ignored (the structs no longer deny them).
pub(crate) fn parse_section<T: serde::de::DeserializeOwned>(
    item: &toml_edit::Item,
) -> Option<T> {
    toml_edit::de::from_str(&item.to_string()).ok()
}

/// The theme directory: `~/.config/jetfish/themes`. Drop `.toml` theme files
/// here (see the iTerm2-Color-Schemes conversions) and name them in the
/// config's `[theme] name = "..."`.
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
