// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::io::Write as _;

use bases::Base64;
use glam::{USizeVec2, UVec2};

use super::KittyParser;
use super::{decode_kitty_rgba, parse_kitty_meta};
use crate::parser::{ImageCtx, ImageParser, ParseOutcome, ParsedImage};

#[test]
fn u1_materializes_a_virtual_placement() {
    let b64 = Base64::encode(&[
        255_u8, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
    ]);
    let mut parser = KittyParser::default();
    let ctx = ImageCtx { cursor_col: 1, cursor_row: 1, screen_start: 0 };

    // `a=t` transmits a 2×2 RGBA image id 1.
    let transmit = format!("a=t,f=32,s=2,v=2,i=1;{b64}");
    assert!(matches!(
        parser.parse(transmit.as_bytes(), b'G', ctx),
        ParseOutcome::Handled(_)
    ));

    // `a=p,U=1` registers a virtual placement id 7 with a 4×2 box.
    let place = "a=p,U=1,i=1,p=7,c=4,r=2,z=-1;";
    assert!(matches!(
        parser.parse(place.as_bytes(), b'G', ctx),
        ParseOutcome::Handled(_)
    ));

    let img = parser
        .materialize_placeholder(1, 7, USizeVec2::new(3, 5))
        .expect("the virtual placement must materialize");
    assert_eq!(img.id, 1);
    assert_eq!(img.anchor, USizeVec2::new(3, 5));
    assert_eq!(img.cell_size, UVec2::new(4, 2));
    assert_eq!(img.size, UVec2::new(2, 2));
    assert_eq!(img.z_index, -1);
    let px = img.display_size(glam::Vec2::new(9.0, 18.0));
    assert_eq!(px, glam::Vec2::new(36.0, 36.0));

    let img = parser
        .materialize_placeholder(1, 99, USizeVec2::new(0, 0))
        .expect("unknown placement id falls back to image id");
    assert_eq!(img.id, 1);
    assert!(parser.materialize_placeholder(999, 7, USizeVec2::ZERO).is_none());
}

#[test]
fn capability_query_acks_in_kitty_format() {
    let mut parser = KittyParser::default();
    let ctx = ImageCtx { cursor_col: 1, cursor_row: 1, screen_start: 0 };

    // yazi's KGP capability probe: `a=q,t=d,f=24` must answer OK.
    let ParseOutcome::Handled(ParsedImage { reply: Some(ok_reply), .. }) =
        parser.parse(b"i=278941603,s=1,v=1,a=q,t=d,f=24;AAAA", b'G', ctx)
    else {
        panic!("a=q capability probe must produce a reply");
    };
    assert_eq!(
        ok_reply, b"\x1b_Gi=278941603;OK\x1b\\",
        "the OK ack must be `Gi=<id>;OK` (yazi matches status == OK)"
    );

    // yazi's shared-memory probe: `a=q,t=s,f=24` is unsupported, so it
    // must answer ENOENT (anything that is not exactly `OK`).
    let ParseOutcome::Handled(ParsedImage { reply: Some(shm_reply), .. }) =
        parser.parse(b"i=916472805,s=1,v=1,a=q,t=s,f=24,S=3;AAAA", b'G', ctx)
    else {
        panic!("a=q shm probe must produce a reply");
    };
    assert_eq!(
        shm_reply, b"\x1b_Gi=916472805;ENOENT:no such image\x1b\\",
        "the unsupported-medium ack must not be `;OK`"
    );
}

#[test]
fn parses_header_fields() {
    // `a=t` transmits (stores) only, with the full header field set.
    let (meta, data) =
        parse_kitty_meta(b"a=t,f=24,s=10,v=10,i=42,p=7,C=1,X=3,Y=4,z=-1;")
            .unwrap();
    assert!(data.is_empty());
    assert_eq!(meta.id, 42);
    assert_eq!(meta.action, 't');
    assert_eq!(meta.size, UVec2::new(10, 10));
    assert_eq!(meta.format, 24);
    assert_eq!(meta.placement_id, 7);
    assert!(!meta.cursor_move, "C=1 leaves the cursor");
    assert_eq!(meta.cell_offset, UVec2::new(3, 4));
    assert_eq!(meta.z_index, -1);
    assert!(!meta.unicode_placeholder);

    // U=1 selects the Unicode placeholder placement model.
    let (meta, _) = parse_kitty_meta(b"a=T,f=32,s=1,v=1,U=1;").unwrap();
    assert!(meta.unicode_placeholder);
}

#[test]
fn parses_cell_dimensions() {
    // jcode (and other clients) send cell dimensions `c=`/`r=` instead of
    // pixel dimensions `s=`/`v=`. Both forms must parse.
    let (meta, _) = parse_kitty_meta(b"a=T,f=100,c=40,r=12,m=0;").unwrap();
    assert_eq!(meta.cell_size, UVec2::new(40, 12));
    assert_eq!(meta.size, UVec2::ZERO);
}

#[test]
fn decodes_raw_rgba() {
    // 2×2 RGBA, uncompressed (kitty f=32 stores bytes directly).
    let rgba = [
        255_u8, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
    ];
    let b64 = Base64::encode(&rgba);
    let (w, h, out) =
        decode_kitty_rgba(b64.as_bytes(), 32, 2, 2, false).unwrap();
    assert_eq!((w, h), (2, 2));
    assert_eq!(out, rgba);
}

#[test]
fn decodes_raw_rgb_expands_alpha() {
    // f=24 has no alpha channel → α becomes 255.
    let rgb = [255_u8, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255];
    let b64 = Base64::encode(&rgb);
    let (w, h, out) =
        decode_kitty_rgba(b64.as_bytes(), 24, 2, 2, false).unwrap();
    assert_eq!((w, h), (2, 2));
    assert_eq!(
        out,
        [
            255_u8, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255,
            255
        ]
    );
}

#[test]
fn decodes_zlib_compressed_rgba() {
    // kitty `o=z`: the base64 payload is zlib (RFC 1950) of the raw RGBA.
    let rgba = [
        255_u8, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
    ];
    let mut encoder = flate2::write::ZlibEncoder::new(
        Vec::new(),
        flate2::Compression::default(),
    );
    encoder.write_all(&rgba).unwrap();
    let compressed = encoder.finish().unwrap();
    let b64 = Base64::encode(&compressed);
    let (w, h, out) =
        decode_kitty_rgba(b64.as_bytes(), 32, 2, 2, true).unwrap();
    assert_eq!((w, h), (2, 2));
    assert_eq!(out, rgba);
}
