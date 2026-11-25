// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

#[test]
fn inline_theme_resolves() {
    let cfg: Config = toml_edit::de::from_str(r##"
        [theme]
        foreground = "#ffffff"
        background = "#000000"
        palette = ["#000000", "#ff0000", "#00ff00", "#0000ff", "#ffff00", "#ff00ff", "#00ffff", "#ffffff", "#808080", "#ff8080", "#80ff80", "#8080ff", "#ffff80", "#ff80ff", "#80ffff", "#cccccc"]
    "##)
        .unwrap();
    let resolved = cfg.theme.resolve(&[]);
    assert_eq!(resolved.foreground, Some(0xffffffff));
    assert_eq!(resolved.background, Some(0x000000ff));
    assert_eq!(resolved.ansi[1], 0xff0000ff);
}

#[test]
fn config_unknown_keys_are_ignored() {
    let cfg: Config = toml_edit::de::from_str("bogus = 1").unwrap();
    assert_eq!(cfg.font.size, jetfish_protocol::Font::DEFAULT.size);
}

#[test]
fn from_toml_tolerates_invalid_keys_and_applies_valid_ones() {
    let cfg = Config::from_toml(
        r##"
        bogus = "ignored"
        bold_is_bright = true

        [font]
        family = "JetBrains Mono"
        size = 18
        bogus_font_key = true

        [window]
        opacity = "oops"

        [debounce]
        interactive_gap_ms = 99

        [theme]
        foreground = "#abcdef"
        "##,
        Path::new("<test>"),
    );
    assert!(cfg.bold_is_bright);
    assert_eq!(cfg.font.family.as_deref(), Some("JetBrains Mono"));
    assert_eq!(cfg.font.size, 18.0);
    assert_eq!(cfg.window.opacity, 1.0, "invalid window section falls back");
    assert_eq!(cfg.debounce.interactive_gap_ms, 99);
    assert_eq!(cfg.resolve_theme().foreground, Some(0xabcdefff));
}

#[test]
fn negative_padding_is_clamped_to_zero() {
    let cfg: Config = toml_edit::de::from_str(
        "[window.padding]\ntop = -10\nleft = -5\nright = 3\nbottom = 0\n",
    )
    .unwrap();
    let p = cfg.window.padding.clamped();
    assert_eq!((p.top, p.left, p.right, p.bottom), (0.0, 0.0, 3.0, 0.0));
}

#[test]
fn missing_sections_resolve_to_compiled_defaults() {
    // An empty config (and `Config::default()`) must NOT fall back to
    // f32/bool zero values: the layout depends on the real compiled
    // defaults (font size, opacity, ligatures).
    let cfg: Config = toml_edit::de::from_str("").unwrap();
    assert_eq!(cfg.font.size, jetfish_protocol::Font::DEFAULT.size);
    assert!(cfg.font.ligatures, "ligatures default ON");
    assert_eq!(cfg.window.opacity, 1.0);
    assert_eq!(
        cfg.debounce.interactive_gap_ms,
        jetfish_protocol::INTERACTIVE_GAP_MS
    );
    assert_eq!(
        cfg.debounce.stream_relayout_ms,
        jetfish_protocol::STREAM_RELAYOUT_MS
    );
}
