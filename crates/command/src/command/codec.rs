// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Terminal-sequence codec: [`EncodeSeq`] serializes a [`Command`] to
//! canonical bytes, [`DecodeSeq`] parses a raw dispatch back; they are inverses
//! for textual commands.

use compact_str::{format_compact, CompactString};
use smallvec::SmallVec;

use vtty::{format_osc_color_reply, format_osc_palette_reply};
use vtty::{Charset, Event, SgrFlags, TermFlags};

use jetfish_protocol::Progress;

use super::{
    ColorCommand, Command, Csi, Dcs, DefaultColor, Marker, Move, Osc, OscCommand, Query,
    WindowCommand,
};

/// The encoded byte buffer — inline [`SmallVec`] for the common short
/// CSI/ESC sequences; long OSC/DCS payloads spill to the heap.
pub type Encoded = SmallVec<u8, 8>;

/// Encode a command into its canonical escape-sequence bytes.
pub trait EncodeSeq {
    /// The CSI/ESC/OSC sequence (including the `ESC` introducer).
    fn encode(self) -> Encoded;
}

/// Decode a raw CSI/ESC/OSC dispatch into a semantic [`Command`].
pub trait DecodeSeq {
    /// `None` when the sequence is unhandled/ignored.
    fn decode(self) -> Option<Command>;
}

impl EncodeSeq for (usize, Charset) {
    fn encode(self) -> Encoded {
        let (g, charset) = self;
        let mut out = Encoded::new();
        Event::EscDispatch {
            intermediates: SmallVec::from(&[Marker::from_index(g).to_u8()]),
            byte: charset.to_u8(),
        }
        .write_bytes(&mut out);
        out
    }
}

impl EncodeSeq for Command {
    fn encode(self) -> Encoded {
        match self {
            Self::Move(m) => m.encode(),
            Self::EraseDisplay(n) => Csi::build(None, &[n as i32], None, b'J'),
            Self::EraseLine(n) => Csi::build(None, &[n as i32], None, b'K'),
            Self::SelectiveEraseDisplay(n) => Csi::build(Some(b"?"), &[n as i32], None, b'J'),
            Self::SelectiveEraseLine(n) => Csi::build(Some(b"?"), &[n as i32], None, b'K'),
            Self::SelectiveEraseChars(n) => Csi::build(Some(b"?"), &[n as i32], None, b'X'),
            Self::Decsca(n) => Csi::build(None, &[n as i32], Some(b"\""), b'q'),
            Self::Sgr(sgr) => {
                let mut out = Encoded::new();
                sgr.write_bytes(&mut out);
                out
            }
            Self::SetMode { mode, on } => encode_mode(mode, on),
            Self::SetModes { modes, on } => {
                // Rebuild the combined form (CSI ?1006;1000h) so a
                // multi-parameter mode sequence round-trips as ONE command.
                let mut codes = SmallVec::<i32, 4>::new();
                let mut private = None;
                for &mode in &modes {
                    let Some((code, p)) = mode.mode_code() else {
                        continue;
                    };
                    if private.is_some_and(|prev| prev != p) {
                        // Mixed ANSI/private params — fall back to one
                        // sequence per mode.
                        let mut out = Encoded::new();
                        for &m in &modes {
                            out.extend(encode_mode(m, on));
                        }
                        return out;
                    }
                    private = Some(p);
                    codes.push(code);
                }
                let prefix = (private == Some(true)).then_some(b"?" as &[u8]);
                Csi::build(prefix, &codes, None, if on { b'h' } else { b'l' })
            }
            Self::SaveModes(codes) => Csi::build(Some(b"?"), codes.as_slice(), None, b's'),
            Self::RestoreModes(codes) => Csi::build(Some(b"?"), codes.as_slice(), None, b'u'),
            Self::ScrollUp(n) => Csi::build(None, &[n as i32], None, b'S'),
            Self::ScrollDown(n) => Csi::build(None, &[n as i32], None, b'T'),
            Self::SetScrollRegion { top, bottom } => {
                Csi::build(None, &[(top + 1) as i32, (bottom + 1) as i32], None, b'r')
            }
            Self::SetLeftRightMargins { left, right } => {
                let mut params = SmallVec::<i32, 2>::new();
                match (left, right) {
                    (Some(l), Some(r)) => {
                        params.extend([(l + 1) as i32, (r + 1) as i32]);
                    }
                    (Some(l), None) => params.push((l + 1) as i32),
                    (None, Some(r)) => params.extend([-2, (r + 1) as i32]),
                    (None, None) => {}
                }
                Csi::build(None, &params, None, b's')
            }
            Self::TabSet => SmallVec::from([0x1b, b'H']),
            Self::TabClear(n) => Csi::build(None, &[n as i32], None, b'g'),
            Self::TabControl(n) => Csi::build(None, &[n as i32], None, b'W'),
            Self::TabEvery8 => Csi::build(Some(b"?"), &[5], None, b'W'),
            Self::ForwardTab(n) => Csi::build(None, &[n as i32], None, b'I'),
            Self::BackwardTab(n) => Csi::build(None, &[n as i32], None, b'Z'),
            Self::AlignmentTest => SmallVec::from([0x1b, b'#', b'8']),
            Self::EraseChars(n) => Csi::build(None, &[n as i32], None, b'X'),
            Self::InsertChars(n) => Csi::build(None, &[n as i32], None, b'@'),
            Self::DeleteChars(n) => Csi::build(None, &[n as i32], None, b'P'),
            Self::InsertLines(n) => Csi::build(None, &[n as i32], None, b'L'),
            Self::DeleteLines(n) => Csi::build(None, &[n as i32], None, b'M'),
            Self::Repeat(n) => Csi::build(None, &[n as i32], None, b'b'),
            Self::ScrollRight(n) => Csi::build(None, &[n as i32], Some(b" "), b'A'),
            Self::ScrollLeft(n) => Csi::build(None, &[n as i32], Some(b" "), b'@'),
            Self::InsertColumns(n) => Csi::build(None, &[n as i32], Some(b"'"), b'}'),
            Self::DeleteColumns(n) => Csi::build(None, &[n as i32], Some(b"'"), b'~'),
            Self::EraseRect {
                top,
                left,
                bottom,
                right,
            } => Csi::build(
                None,
                &[top as i32, left as i32, bottom as i32, right as i32],
                Some(b"$"),
                b'z',
            ),
            Self::FillRect {
                ch,
                top,
                left,
                bottom,
                right,
            } => Csi::build(
                None,
                &[
                    ch as u32 as i32,
                    top as i32,
                    left as i32,
                    bottom as i32,
                    right as i32,
                ],
                Some(b"$"),
                b'x',
            ),
            Self::CopyRect {
                top,
                left,
                bottom,
                right,
                page,
            } => Csi::build(
                None,
                &[top as i32, left as i32, bottom as i32, right as i32, page],
                Some(b"$"),
                b'v',
            ),
            Self::ChangeRectAttrs {
                top,
                left,
                bottom,
                right,
                set,
                mask,
                value,
            } => {
                let mut params = vec![top as i32, left as i32, bottom as i32, right as i32];
                let codes = [
                    (SgrFlags::BOLD, 1_i32, 22_i32),
                    (SgrFlags::UNDERLINE, 4, 24),
                    (SgrFlags::BLINK, 5, 25),
                    (SgrFlags::INVERSE, 7, 27),
                ];
                for (flag, on_code, off_code) in codes {
                    if !mask.contains(flag) {
                        continue;
                    }
                    params.push(if set {
                        if value.contains(flag) {
                            on_code
                        } else {
                            off_code
                        }
                    } else {
                        on_code
                    });
                }
                Csi::build(None, &params, Some(b"$"), if set { b'r' } else { b't' })
            }
            Self::SaveCursor => SmallVec::from([0x1b, b'7']),
            Self::AnsiSaveCursor => Csi::build(None, &[], None, b's'),
            Self::RestoreCursor => SmallVec::from([0x1b, b'8']),
            Self::PushColors => Csi::build(None, &[], Some(b"#"), b'P'),
            Self::PopColors => Csi::build(None, &[], Some(b"#"), b'Q'),
            Self::ReportColors => Csi::build(None, &[], Some(b"#"), b'R'),
            Self::SetKeypad(on) => SmallVec::from([0x1b, if on { b'=' } else { b'>' }]),
            Self::Scp(n) => Csi::build(None, &[n as i32], Some(b" "), b'k'),
            Self::CharsetDesignate { g, charset } => (g, charset).encode(),
            Self::LockingShift(g) => SmallVec::from(match g {
                2 => [0x1b, b'n'],
                3 => [0x1b, b'o'],
                _ => [0x1b, b'n'], // unreachable: only LS2/LS3 decode to LockingShift
            }),
            Self::Reset => SmallVec::from([0x1b, b'c']),
            Self::SoftReset => Csi::build(None, &[], Some(b"!"), b'p'),
            Self::Query(q) => q.encode(),
            Self::CursorShape(n) => Csi::build(None, &[n], Some(b" "), b'q'),
            Self::ModifyOtherKeys(n) => Csi::build(Some(b">"), &[4, n as i32], None, b'm'),
            Self::SetTitleMode(n) => Csi::build(Some(b">"), &[n as i32], None, b't'),
            Self::Osc(o) => o.encode(),
            Self::KittyKeyboardPush(n) => Csi::build(Some(b">"), &[n as i32], None, b'u'),
            Self::KittyKeyboardSet { flags, mode } => {
                Csi::build(Some(b"="), &[flags as i32, mode as i32], None, b'u')
            }
            Self::KittyKeyboardPop(n) => Csi::build(Some(b"<"), &[n as i32], None, b'u'),
            // Image payloads are decode-only; they don't round-trip.
            #[cfg(feature = "kitty")]
            Self::KittyImage(_) => SmallVec::new(),
            #[cfg(feature = "iterm2")]
            Self::Iterm2Image(_) => SmallVec::new(),
            Self::Window(w) => w.encode(),
        }
    }
}

impl EncodeSeq for Move {
    fn encode(self) -> Encoded {
        match self {
            Self::To(pos) => {
                Csi::build(None, &[(pos.y + 1) as i32, (pos.x + 1) as i32], None, b'H')
            }
            Self::Up(n) => Csi::build(None, &[n as i32], None, b'A'),
            Self::Down(n) => Csi::build(None, &[n as i32], None, b'B'),
            Self::Right(n) => Csi::build(None, &[n as i32], None, b'C'),
            Self::Left(n) => Csi::build(None, &[n as i32], None, b'D'),
            Self::Col(col) => Csi::build(None, &[(col + 1) as i32], None, b'G'),
            Self::Row(row) => Csi::build(None, &[(row + 1) as i32], None, b'd'),
            Self::NextLine(n) => Csi::build(None, &[n as i32], None, b'E'),
            Self::PrevLine(n) => Csi::build(None, &[n as i32], None, b'F'),
            Self::NewLine => SmallVec::from([0x1b, b'E']),
            Self::Index => SmallVec::from([0x1b, b'D']),
            Self::ReverseIndex => SmallVec::from([0x1b, b'M']),
        }
    }
}

impl EncodeSeq for Query {
    fn encode(self) -> Encoded {
        match self {
            Self::DeviceStatus(n) => Csi::build(None, &[n as i32], None, b'n'),
            Self::ExtendedCursorPosition => Csi::build(Some(b"?"), &[6], None, b'n'),
            Self::DeviceAttrs(kind) => match kind {
                2 => Csi::build(Some(b">"), &[], None, b'c'),
                3 => Csi::build(Some(b"="), &[], None, b'c'),
                _ => Csi::build(None, &[], None, b'c'),
            },
            Self::Mode { param, private } => {
                if private {
                    Csi::build(Some(b"?"), &[param], Some(b"$"), b'p')
                } else {
                    Csi::build(None, &[param], Some(b"$"), b'p')
                }
            }
            Self::Setting(setting) => {
                Dcs::new(SmallVec::new(), 0, vec![b'$'], b'q', setting).encode()
            }
            Self::Termcap(cap) => Dcs::new(
                SmallVec::new(),
                0,
                vec![b'+'],
                b'q',
                vtty::const_hex::encode(&cap).into_bytes(),
            )
            .encode(),
            Self::Version => Csi::build(Some(b">"), &[], None, b'q'),
            Self::KittyKeyboard => Csi::build(Some(b"?"), &[], None, b'u'),
            Self::ModifyOtherKeys => Csi::build(Some(b"?"), &[4], None, b'm'),
            Self::WindowSize { reply_code } => Csi::build(None, &[reply_code as i32], None, b't'),
            Self::WindowPosition => Csi::build(None, &[13], None, b't'),
            Self::WindowState => Csi::build(None, &[11], None, b't'),
            Self::DefaultColor(target) => {
                let code = match target {
                    DefaultColor::Foreground => b"10".as_slice(),
                    DefaultColor::Background => b"11".as_slice(),
                    DefaultColor::Cursor => b"12".as_slice(),
                };
                Osc::new(&[code, b"?"]).encode()
            }
            Self::PaletteColor(i) => Osc::new(&[b"4", i.to_string().as_bytes(), b"?"]).encode(),
            Self::ColorScheme => Csi::build(Some(b"?"), &[996], None, b'n'),
            Self::Clipboard(sel) => Osc::new(&[b"52", &[sel], b"?"]).encode(),
            Self::ChecksumRect {
                id,
                top,
                left,
                bottom,
                right,
            } => Csi::build(
                None,
                &[id, 0, top as i32, left as i32, bottom as i32, right as i32],
                Some(b"*"),
                b'y',
            ),
        }
    }
}

impl EncodeSeq for OscCommand {
    fn encode(self) -> Encoded {
        match self {
            Self::SetTitle(t) => Osc::new(&[b"0", t.as_bytes()]).encode(),
            Self::SetClipboard(s) => Osc::new(&[b"52", b"c", s.as_bytes()]).encode(),
            Self::Progress(p) => match p {
                Progress::Indeterminate => Osc::new(&[b"9", b"4", b"0"]).encode(),
                Progress::Percent(n) => {
                    let s = n.to_string();
                    Osc::new(&[b"9", b"4", b"1", s.as_bytes()]).encode()
                }
                Progress::Done => Osc::new(&[b"9", b"4", b"2"]).encode(),
                Progress::Error => Osc::new(&[b"9", b"4", b"3"]).encode(),
                Progress::Hidden => Osc::new(&[b"9", b"4"]).encode(),
            },
            Self::SetTabTitle(title) => match title {
                Some(t) => Osc::new(&[b"9", b"3", t.as_bytes()]).encode(),
                None => Osc::new(&[b"9", b"3"]).encode(),
            },
            Self::Notify(s) => Osc::new(&[b"9", s.as_bytes()]).encode(),
            Self::SetHyperlink(link) => match link {
                Some(h) => {
                    if let Some(id) = &h.id {
                        let p = format!("id={id}");
                        Osc::new(&[b"8", p.as_bytes(), h.uri.as_bytes()]).encode()
                    } else {
                        Osc::new(&[b"8", b"", h.uri.as_bytes()]).encode()
                    }
                }
                None => Osc::new(&[b"8"]).encode(),
            },
            Self::SetCwd(s) => Osc::new(&[b"7", s.as_bytes()]).encode(),
            Self::ShellIntegration(p) => p.encode(),
            Self::Color(c) => c.encode(),
            Self::Notification(n) => {
                Osc::new(&[b"777", b"notify", n.title.as_bytes(), n.body.as_bytes()]).encode()
            }
            Self::MouseCursor(name) => Osc::new(&[b"22", name.as_bytes()]).encode(),
            Self::CursorShape(n) => {
                let s = format!("CursorShape={n}");
                Osc::new(&[b"50", s.as_bytes()]).encode()
            }
            Self::KittyColor(ops) => super::kitty_color::encode(&ops),
        }
    }
}

impl EncodeSeq for ColorCommand {
    fn encode(self) -> Encoded {
        match self {
            Self::SetDefault { target, rgb } => {
                Encoded::from(format_osc_color_reply(target.as_set(), rgb).as_bytes())
            }
            Self::ResetDefault(target) => Osc::new(&[target.as_reset()]).encode(),
            Self::SetHighlight { target, rgb } => {
                Encoded::from(format_osc_color_reply(target.as_highlight_set(), rgb).as_bytes())
            }
            Self::ResetHighlight(target) => Osc::new(&[target.as_highlight_reset()]).encode(),
            Self::SetPalette { index, rgb } => {
                Encoded::from(format_osc_palette_reply(index, rgb).as_bytes())
            }
            Self::SetPaletteBatch(entries) => {
                let mut parts: Vec<Vec<u8>> = vec![b"4".to_vec()];
                for (idx, rgb) in entries {
                    parts.push(idx.to_string().into());
                    parts.push(rgb_spec(rgb).into());
                }
                let refs: Vec<&[u8]> = parts.iter().map(Vec::as_slice).collect();
                Osc::new(&refs).encode()
            }
            Self::ResetPalette(index) => match index {
                Some(i) => Osc::new(&[b"104", i.to_string().as_bytes()]).encode(),
                None => Osc::new(&[b"104"]).encode(),
            },
        }
    }
}

impl EncodeSeq for WindowCommand {
    fn encode(self) -> Encoded {
        match self {
            Self::ResizeTextArea { rows, cols } => {
                Csi::build(None, &[8, rows as i32, cols as i32], None, b't')
            }
            Self::PushTitle => Csi::build(None, &[22], None, b't'),
            Self::PopTitle => Csi::build(None, &[23], None, b't'),
        }
    }
}

/// Format a packed 0xRRGGBBAA as the xterm `rgb:RRRR/GGGG/BBBB` spec.
fn rgb_spec(rgb: u32) -> CompactString {
    let [r, g, b] = vtty::u32_to_rgb(rgb);
    format_compact!("rgb:{r:02x}{r:02x}/{g:02x}{g:02x}/{b:02x}{b:02x}")
}

/// Encode a [`TermFlags`] mode set/reset as one `CSI … h/l` sequence per flag.
fn encode_mode(mode: TermFlags, on: bool) -> Encoded {
    let mut out = Encoded::new();
    for bit in 0..64_u32 {
        let flag = TermFlags::from_bits_truncate(1_u64 << bit);
        if flag.is_empty() || !mode.contains(flag) {
            continue;
        }
        let Some((code, private)) = flag.mode_code() else {
            continue;
        };
        let mut intermediates = SmallVec::<u8, 2>::new();
        if private {
            intermediates.push(b'?');
        }
        Event::CsiDispatch {
            params: SmallVec::from(&[code]),
            sep: 0,
            intermediates,
            final_byte: if on { b'h' } else { b'l' },
        }
        .write_bytes(&mut out);
    }
    out
}

#[cfg(test)]
#[path = "../../tests/command/codec.rs"]
mod tests;
