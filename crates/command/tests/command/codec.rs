// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;
use crate::command::Sgr;
use crate::command::{Hyperlink, KittyColorKey, KittyColorOp};
use crate::commands::Commands;

#[test]
fn roundtrip_commands() {
    let cases = vec![
        Command::Move(Move::To(jetfish_protocol::CursorPos::new(7, 3))),
        Command::Move(Move::NewLine),
        Command::Move(Move::ReverseIndex),
        Command::EraseDisplay(2),
        Command::SelectiveEraseLine(1),
        Command::Decsca(2),
        Command::Sgr(Sgr::new(SmallVec::from([1, 38, 2, 10, 20, 30]), 0)),
        Command::Sgr(Sgr::new(
            SmallVec::from([38, 2, 0, 10, 20, 30]),
            0b11111,
        )),
        Command::SetMode { mode: TermFlags::BRACKETED_PASTE, on: true },
        Command::SetMode { mode: TermFlags::INSERT, on: false },
        Command::SetModes {
            modes: vec![TermFlags::MOUSE_SGR, TermFlags::MOUSE_NORMAL],
            on: true,
        },
        Command::ScrollUp(3),
        Command::SetScrollRegion { top: 2, bottom: 20 },
        Command::SetLeftRightMargins { left: Some(0), right: Some(79) },
        Command::TabSet,
        Command::TabEvery8,
        Command::EraseChars(4),
        Command::Repeat(9),
        Command::InsertColumns(2),
        Command::EraseRect { top: 1, left: 2, bottom: 3, right: 4 },
        Command::FillRect { ch: 'x', top: 1, left: 2, bottom: 3, right: 4 },
        Command::SaveCursor,
        Command::PushColors,
        Command::SetKeypad(true),
        Command::Scp(1),
        Command::CharsetDesignate { g: 0, charset: Charset::Dec },
        Command::Reset,
        Command::SoftReset,
        Command::Query(Query::DeviceStatus(6)),
        Command::Query(Query::DefaultColor(DefaultColor::Background)),
        Command::Query(Query::PaletteColor(3)),
        Command::Query(Query::ColorScheme),
        Command::CursorShape(2),
        Command::ModifyOtherKeys(1),
        Command::SaveModes(vec![12, 25]),
        Command::RestoreModes(vec![12]),
        Command::Osc(OscCommand::SetTitle("hi".into())),
        Command::Osc(OscCommand::SetHyperlink(Some(Hyperlink {
            uri: "https://x".into(),
            id: None,
        }))),
        Command::Osc(OscCommand::SetHyperlink(Some(Hyperlink {
            uri: "https://x/a;b".into(),
            id: Some("grp1".into()),
        }))),
        Command::Osc(OscCommand::SetHyperlink(None)),
        Command::Osc(OscCommand::Progress(Progress::Percent(50))),
        Command::Osc(OscCommand::Progress(Progress::Indeterminate)),
        Command::Osc(OscCommand::Progress(Progress::Done)),
        Command::Osc(OscCommand::Color(ColorCommand::SetDefault {
            target: DefaultColor::Foreground,
            rgb: 0xff0000ff,
        })),
        Command::Osc(OscCommand::Color(ColorCommand::SetHighlight {
            target: DefaultColor::Background,
            rgb: 0x112233ff,
        })),
        Command::Osc(OscCommand::Color(ColorCommand::SetPaletteBatch(vec![
            (5, 0xabcdefff),
            (6, 0x123456ff),
        ]))),
        Command::Osc(OscCommand::Color(ColorCommand::ResetPalette(None))),
        Command::Osc(OscCommand::KittyColor(vec![
            KittyColorOp::Query(KittyColorKey::Foreground),
            KittyColorOp::Set {
                key: KittyColorKey::Palette(7),
                rgb: Some(0x112233ff),
            },
            KittyColorOp::Set { key: KittyColorKey::Cursor, rgb: None },
            KittyColorOp::Reset(KittyColorKey::Background),
        ])),
        Command::Window(WindowCommand::PushTitle),
        Command::KittyKeyboardPush(3),
    ];
    for cmd in cases {
        let bytes = cmd.clone().encode();
        let mut cmds = Commands::new();
        let mut parser = vtty::Parser::new();
        parser.advance(&mut cmds, &bytes);
        let decoded: Vec<Command> = cmds.drain().collect();
        assert_eq!(
            decoded,
            vec![cmd.clone()],
            "round-trip failed for {cmd:?} (bytes {bytes:?})"
        );
    }
}
