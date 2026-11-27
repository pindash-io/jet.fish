// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

use vtty::hex_to_rgba_u32 as parse_hex;

#[test]
fn parse_hex_handles_hash_and_bare() {
    assert_eq!(parse_hex("#ff0000"), Some(0xff0000ff));
    assert_eq!(parse_hex("ff0000"), Some(0xff0000ff));
    assert_eq!(parse_hex("fff"), Some(0xffffffff));
    assert_eq!(parse_hex("#f00"), Some(0xff0000ff));
    assert_eq!(parse_hex("#0af"), Some(0x00aaffff));
    assert_eq!(parse_hex("ff"), None);
}

#[test]
fn user_theme_dir_overrides_bundled() {
    let dir = std::env::temp_dir().join("jetfish-test-themes");
    std::fs::create_dir_all(&dir).unwrap();
    let custom = dir.join("My-Theme.toml");
    std::fs::write(
        &custom,
        r##"name = "My Theme"
palette = [
    "#111111", "#222222", "#333333", "#444444",
    "#555555", "#666666", "#777777", "#888888",
    "#999999", "#aaaaaa", "#bbbbbb", "#cccccc",
    "#dddddd", "#eeeeee", "#ffffff", "#000000",
]
"##,
    )
    .unwrap();

    let cfg: crate::Config = toml_edit::de::from_str(
        r##"[theme]
name = "My-Theme"
foreground = "#abcdef"
"##,
    )
    .unwrap();
    let resolved = cfg.theme.resolve(std::slice::from_ref(&dir));
    assert_eq!(resolved.ansi[0], 0x111111ff);
    assert_eq!(resolved.ansi[1], 0x222222ff);
    // inline foreground wins over the named theme
    assert_eq!(resolved.foreground, Some(0xabcdefff));

    std::fs::remove_dir_all(&dir).unwrap();
}
