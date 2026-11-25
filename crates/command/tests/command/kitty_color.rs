// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

#[test]
fn parse_query_set_reset_dynamic() {
    let ops = parse(&[
        b"foreground=?".as_slice(),
        b"cursor=rgb:ff/00/00".as_slice(),
        b"background".as_slice(),
        b"cursor=".as_slice(),
        b"255=?".as_slice(),
        b"unknown=?".as_slice(),
    ])
    .expect("valid ops");
    assert_eq!(
        ops,
        vec![
            KittyColorOp::Query(KittyColorKey::Foreground),
            KittyColorOp::Set {
                key: KittyColorKey::Cursor,
                rgb: Some(0xff00_00ff)
            },
            KittyColorOp::Reset(KittyColorKey::Background),
            KittyColorOp::Set { key: KittyColorKey::Cursor, rgb: None },
            KittyColorOp::Query(KittyColorKey::Palette(255)),
            KittyColorOp::Unknown("unknown".into()),
        ]
    );
}

#[test]
fn parse_unknown_key_is_an_op() {
    assert_eq!(
        parse(&[b"nonsense=?".as_slice()]),
        Some(vec![KittyColorOp::Unknown("nonsense".into())])
    );
}

#[test]
fn encode_round_trip() {
    let ops = vec![
        KittyColorOp::Query(KittyColorKey::Foreground),
        KittyColorOp::Set {
            key: KittyColorKey::Cursor,
            rgb: Some(0xff00_00ff),
        },
        KittyColorOp::Reset(KittyColorKey::Background),
        KittyColorOp::Set { key: KittyColorKey::Cursor, rgb: None },
        KittyColorOp::Query(KittyColorKey::Palette(255)),
        KittyColorOp::Unknown("nonsense".into()),
    ];
    let encoded = encode(&ops);
    assert_eq!(
        &encoded[..],
        b"\x1b]21;foreground=?;cursor=rgb:ffff/0000/0000;background;cursor=;255=?;nonsense=?\x1b\\"
    );
    // The encoded bytes decode back to the same ops.
    let pairs = &[
        b"21".as_slice(),
        b"foreground=?".as_slice(),
        b"cursor=rgb:ffff/0000/0000".as_slice(),
        b"background".as_slice(),
        b"cursor=".as_slice(),
        b"255=?".as_slice(),
        b"nonsense=?".as_slice(),
    ];
    assert_eq!(parse(&pairs[1..]), Some(ops));
}

#[test]
fn reply_formats_dynamic_and_unknown() {
    let entries = [
        ReplyEntry::Color(KittyColorKey::Foreground, Some(0xff_ff_ff_ff)),
        ReplyEntry::Color(KittyColorKey::CursorText, None),
        ReplyEntry::Unknown("nonsense".into()),
    ];
    assert_eq!(
        reply(&entries).unwrap(),
        "\x1b]21;foreground=rgb:ffff/ffff/ffff;cursor_text=;unknown=bm9uc2Vuc2U=\x1b\\"
    );
}

#[test]
fn resolve_maps_keys() {
    let defaults = Defaults {
        fg: 0x00_11_22_ff,
        bg: 0x33_44_55_ff,
        cursor: 0x66_77_88_ff,
    };
    let highlight = Highlight { fg: Some(0x99_aa_bb_ff), bg: None };
    let mut palette = Palette::new();
    palette.set_entry(7, 0x12_34_56_ff);
    assert_eq!(
        resolve(KittyColorKey::Foreground, &defaults, &highlight, &palette),
        Some(0x00_11_22_ff)
    );
    assert_eq!(
        resolve(
            KittyColorKey::SelectionBackground,
            &defaults,
            &highlight,
            &palette
        ),
        None
    );
    assert_eq!(
        resolve(KittyColorKey::Palette(7), &defaults, &highlight, &palette),
        Some(0x12_34_56_ff)
    );
}
