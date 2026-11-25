// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

/// The fluent helpers must buffer commands in push order, and `apply_to`
/// must drain the queue into the target in that order.
#[test]
fn fluent_helpers_buffer_and_apply_in_order() {
    struct Recorder(Vec<Command>);
    impl Apply for Recorder {
        fn apply(&mut self, cmd: Command) {
            self.0.push(cmd);
        }

        fn decode_ctx(&self) -> CsiCtx {
            CsiCtx::default()
        }

        fn osc_policy(&self) -> Osc52 {
            Osc52::default()
        }

        fn style(&self) -> Style {
            Style::DEFAULT
        }

        fn set_style(&mut self, _style: Style) {}

        fn palette_mut(&mut self) -> &mut Palette {
            unreachable!("Recorder never applies SGR effects")
        }

        fn color_stack_mut(&mut self) -> &mut Vec<(ColorId, ColorId)> {
            unreachable!("Recorder never applies color-stack commands")
        }
    }

    let mut commands = Commands::new();
    commands.move_to(1, 2).erase_display(2).scroll_up(3).save_cursor();
    assert_eq!(commands.len(), 4);

    let mut target = Recorder(Vec::new());
    commands.apply_to(&mut target);
    assert!(commands.is_empty());
    assert_eq!(target.0.len(), 4);
    assert!(matches!(
        target.0[0],
        Command::Move(Move::To(pos)) if pos.x == 2 && pos.y == 1
    ));
    assert!(matches!(target.0[1], Command::EraseDisplay(2)));
    assert!(matches!(target.0[2], Command::ScrollUp(3)));
    assert!(matches!(target.0[3], Command::SaveCursor));
}

/// As a `Perform` sink, the parser decodes a byte stream into commands
/// without a screen; text is dropped and CSI/ESC/OSC are captured.
#[test]
fn parses_bytes_into_commands_without_a_screen() {
    let mut commands = Commands::new();
    let mut parser = vtty::Parser::new();
    parser.advance(&mut commands, b"\x1b[2J\x1b[3;5H\x1b[K hello \x1b[m");
    assert_eq!(commands.len(), 4);
    assert!(matches!(commands.iter().next(), Some(Command::EraseDisplay(2))));
    assert!(matches!(
        commands.iter().nth(1),
        Some(Command::Move(Move::To(pos))) if pos.x == 4 && pos.y == 2
    ));
    assert!(matches!(commands.iter().nth(2), Some(Command::EraseLine(0))));
    assert!(matches!(commands.iter().nth(3), Some(Command::Sgr(_))));
}
