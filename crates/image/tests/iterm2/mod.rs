// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

#[test]
fn parses_inline_png() {
    // "AAAA" base64-decodes to 3 bytes; PNG validity is the host's job.
    let params: [&[u8]; 4] =
        [b"1337", b"File=name=hello.png", b"size=3", b"inline=1:AAAA"];
    let img = parse_iterm2(&params).unwrap();
    assert_eq!(img.name.as_deref(), Some("hello.png"));
    assert!(!img.do_not_move_cursor);
    assert!(img.preserve_aspect_ratio);
    assert_eq!(img.bytes, vec![0, 0, 0]);
}

#[test]
fn single_parameter_form() {
    let params: [&[u8]; 2] = [b"1337", b"File=inline=1:AAAA"];
    let img = parse_iterm2(&params).unwrap();
    assert_eq!(img.bytes, vec![0, 0, 0]);
}

#[test]
fn resize_units() {
    assert_eq!(parse_resize(b"10"), Some(Resize::Cells(10)));
    assert_eq!(parse_resize(b"50px"), Some(Resize::Pixels(50)));
    assert_eq!(parse_resize(b"25%"), Some(Resize::Percent(25)));
    assert_eq!(parse_resize(b"auto"), Some(Resize::Auto));
    assert_eq!(parse_resize(b""), Some(Resize::Auto));
    assert_eq!(parse_resize(b"abc"), None);
}

#[test]
fn rejects_non_inline() {
    let params: [&[u8]; 2] = [b"1337", b"File=inline=0:AAAA"];
    assert!(parse_iterm2(&params).is_none());
}
