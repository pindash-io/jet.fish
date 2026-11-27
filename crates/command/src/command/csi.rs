// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! CSI (`ESC [`) decoder — shared, screen-agnostic.

use glam::USizeVec2;
use smallvec::SmallVec;

use vtty::{Event, TermFlags};

use jetfish_protocol::{CursorPos, Grid};

use super::codec::{DecodeSeq, EncodeSeq, Encoded};
use super::{Command, Move, Query, Sgr, WindowCommand};

/// Defensive cap on REP (`CSI Ps b`): a single sequence must not drive
/// millions of writes.
const MAX_REPEAT: usize = 10_000;

/// The screen context the CSI decoder needs to normalize coordinates.
#[derive(Clone, Copy, Debug, Default)]
pub struct CsiCtx {
    /// Grid dimensions (`x` = cols, `y` = rows).
    pub size: Grid,
    /// Scroll region (`x` = top, `y` = bottom), origin-mode-relative.
    pub scroll: USizeVec2,
    /// Left/right margins (`x` = left, `y` = right), origin-mode-relative.
    pub margin: USizeVec2,
    /// DECOM origin mode — positions are relative to `scroll.x` / `margin.x`.
    pub decom: bool,
}

/// A raw CSI dispatch (`ESC [ …`).
#[derive(Debug)]
pub struct Csi<'a> {
    /// The CSI parameter values.
    pub params: &'a [i32],
    /// Bit `i` is set when parameter `i` was followed by a colon.
    pub sep: u16,
    /// The intermediate bytes (`?`, `>`, `=`, `"`, ` `, `$`, …).
    pub ints: &'a [u8],
    /// The final byte.
    pub final_byte: u8,
    /// The decode context (grid size / scroll region / origin mode).
    pub ctx: CsiCtx,
}

impl Csi<'_> {
    /// Build a CSI sequence: `ESC [ prefix params suffix final`.
    pub(super) fn build(
        prefix: Option<&[u8]>,
        params: &[i32],
        suffix: Option<&[u8]>,
        final_byte: u8,
    ) -> Encoded {
        Self::build_with_sep(prefix, params, 0, suffix, final_byte)
    }

    /// Build a CSI sequence with the given colon-separator bitfield (`sep` bit
    /// `i` = parameter `i` followed by a colon).
    fn build_with_sep(
        prefix: Option<&[u8]>,
        params: &[i32],
        sep: u16,
        suffix: Option<&[u8]>,
        final_byte: u8,
    ) -> Encoded {
        let mut ints = SmallVec::<u8, 2>::new();
        if let Some(p) = prefix {
            ints.extend_from_slice_copy(p);
        }
        if let Some(s) = suffix {
            ints.extend_from_slice_copy(s);
        }
        Csi { params, sep, ints: &ints, final_byte, ctx: CsiCtx::default() }
            .encode()
    }
}

impl DecodeSeq for Csi<'_> {
    fn decode(self) -> Option<Command> {
        Self::parse(
            self.params,
            self.sep,
            self.ints,
            self.final_byte,
            self.ctx,
        )
    }
}

impl EncodeSeq for Csi<'_> {
    fn encode(self) -> Encoded {
        let mut intermediates = SmallVec::<u8, 2>::new();
        intermediates.extend_from_slice_copy(self.ints);
        let mut out = Encoded::new();
        Event::CsiDispatch {
            params: SmallVec::from(self.params),
            sep: self.sep,
            intermediates,
            final_byte: self.final_byte,
        }
        .write_bytes(&mut out);
        out
    }
}

impl Csi<'_> {
    /// Decode a CSI (`ESC [`) sequence into a [`Command`], or `None` for
    /// unhandled/ignored sequences.
    #[must_use]
    pub fn parse(
        params: &[i32],
        sep: u16,
        ints: &[u8],
        final_byte: u8,
        ctx: CsiCtx,
    ) -> Option<Command> {
        // The private marker (`?`) is always the first intermediate byte when
        // present; a scalar `first()` check is cheaper than `[u8]::contains`
        // (which dispatches to SIMD memchr even for empty/1-byte slices).
        let int = ints.first().copied();
        let private = int == Some(b'?');
        let p0 = params.first().copied().unwrap_or(0);
        let p = |i: usize, def: i32| params.get(i).copied().unwrap_or(def);
        let n = |i: usize| p(i, 1).max(1) as usize;
        let rows = ctx.size.y.max(1) as i32;
        let cols = ctx.size.x.max(1) as i32;
        let dollar = ints.contains(&b'$');

        let cmd = match (final_byte, int) {
            // Cursor position: CSI r ; c H (1-based).
            (b'H' | b'f', _) => {
                // CUP position: 0-based `(col, row)`.
                let mut pos = CursorPos::new(
                    p(1, 1).max(1) as usize - 1,
                    p(0, 1).max(1) as usize - 1,
                );
                if ctx.decom {
                    pos = (pos + CursorPos::new(ctx.margin.x, ctx.scroll.x))
                        .min(CursorPos::new(ctx.margin.y, ctx.scroll.y));
                }
                Move::To(pos).into()
            }
            (b'A', Some(b' ')) => Command::ScrollRight(n(0)), // SR
            (b'A', _) => Move::Up(n(0)).into(),
            (b'B', _) => Move::Down(n(0)).into(),
            (b'C', _) => Move::Right(n(0)).into(),
            (b'D', _) => Move::Left(n(0)).into(),
            // CHA / HPA (`CSI Ps G` / `CSI Ps `): the column is relative to
            // the DECSLRM left margin under origin mode and clamps right.
            (b'G' | b'`', _) => {
                let mut col = p(0, 1).max(1) as usize - 1;
                if ctx.decom {
                    col = (col + ctx.margin.x).min(ctx.margin.y);
                }
                Move::Col(col).into()
            }
            (b'd', _) => {
                let mut row = p(0, 1).max(1) as usize - 1;
                if ctx.decom {
                    row = (row + ctx.scroll.x).min(ctx.scroll.y);
                }
                Move::Row(row).into()
            }
            (b'E', _) => Move::NextLine(n(0)).into(), // CNL
            (b'F', _) => Move::PrevLine(n(0)).into(), // CPL
            (b'a', _) => Move::Right(n(0)).into(),    // HPR
            (b'e', _) => Move::Down(n(0)).into(),     // VPR (same as CUD)
            (b'j', None) => Move::Left(n(0)).into(),  // HPB
            (b'k', None) => Move::Up(n(0)).into(),    // VPB
            (b'J', Some(b'?')) => Command::SelectiveEraseDisplay(p0 as u8),
            (b'J', _) => Command::EraseDisplay(p0 as u8),
            (b'K', Some(b'?')) => Command::SelectiveEraseLine(p0 as u8),
            (b'K', _) => Command::EraseLine(p0 as u8),
            // `CSI > ... m` is kitty keyboard, not SGR — only plain CSI m.
            (b'm', Some(b'>')) if p0 == 4 => {
                // XTerm modifyOtherKeys: `CSI >4;1m` (format 1), `CSI >4;2m`
                // (format 2), `CSI >4;0m` / `CSI >4m` (off). fish enables it
                // when kitty keyboard is unavailable.
                Command::ModifyOtherKeys(p(1, 0) as u8)
            }
            // `CSI ? 4 m` is the modifyOtherKeys QUERY (xterm), not SGR.
            (b'm', Some(b'?')) if p0 == 4 => Query::ModifyOtherKeys.into(),
            (b'm', Some(b'>')) => return None,
            (b'm', _) => Command::Sgr(Sgr::new(SmallVec::from(params), sep)),
            // DECXCPR (`CSI ?6n`) — extended cursor position with the `?`
            // prefix so ultraviolet can disambiguate it from plain CPR.
            (b'n', Some(b'?')) if p0 == 6 => {
                Query::ExtendedCursorPosition.into()
            }
            (b'n', Some(b'?')) if matches!(p0, 996 | 997) => {
                Query::ColorScheme.into()
            }
            (b'n', _) => Query::DeviceStatus(p0 as u8).into(),
            (b'c', Some(b'>')) => Query::DeviceAttrs(2).into(),
            (b'c', Some(b'=')) => Query::DeviceAttrs(3).into(),
            (b'c', _) => Query::DeviceAttrs(1).into(),
            (b'q', Some(b'>')) => Query::Version.into(),
            (b'q', Some(b' ')) => Command::CursorShape(p0),
            (b'q', Some(b'"')) => Command::Decsca(p0 as u8), // DECSCA
            // SCP — set character path (ECMA-48 bidi): `CSI Ps;Ps SP k`.
            (b'k', Some(b' ')) => {
                let path = p0;
                let update = p(1, 0);
                if path > 2 || update > 2 {
                    return None;
                }
                Command::Scp(path as u8)
            }
            // XTRESTORE (`CSI ? Pm u`) vs kitty keyboard query (`CSI ? u`):
            // a param that maps to a real DEC mode is XTRESTORE; the
            // param-less query is the kitty keyboard report.
            (b'u', Some(b'?')) => {
                if params.iter().copied().any(|c| {
                    TermFlags::from_any_mode_code(c) != TermFlags::NONE
                }) {
                    Command::RestoreModes(params.to_vec())
                } else {
                    Query::KittyKeyboard.into()
                }
            }
            // CSI = flags ; mode u — kitty progressive enhancement.
            (b'u', Some(b'=')) => Command::KittyKeyboardSet {
                flags: p0 as u8,
                mode: p(1, 1) as u8,
            },
            (b'u', Some(b'>')) => Command::KittyKeyboardPush(p0 as u8),
            (b'u', Some(b'<')) => {
                Command::KittyKeyboardPop(p(0, 1).max(1) as usize)
            }
            // DECRARA (`CSI Pt;Pl;Pb;Pr;Ps… $ t`) — reverse the bold/
            // underline/blink/reverse attributes in a rectangle.
            (b't', _) if dollar => {
                let (mask, value) = jetfish_protocol::rect_attr_mask(
                    params.get(4..).unwrap_or(&[]),
                    false,
                )?;
                Command::ChangeRectAttrs {
                    top: p(0, 1).max(1) as usize,
                    left: p(1, 1).max(1) as usize,
                    bottom: p(2, rows).max(1) as usize,
                    right: p(3, cols).max(1) as usize,
                    set: false,
                    mask,
                    value,
                }
            }
            // XTITLEMODES (`CSI > Ps t`): 0 = hex, 1 = UTF-8,
            // 2 = unconditional UTF-8.
            (b't', Some(b'>')) if matches!(p0, 0..=2) => {
                Command::SetTitleMode(p0 as u8)
            }
            (b't', _) => match p0 {
                11 => Query::WindowState.into(),
                13 => Query::WindowPosition.into(),
                14 => Query::WindowSize { reply_code: 4 }.into(),
                15 => Query::WindowSize { reply_code: 5 }.into(),
                16 => Query::WindowSize { reply_code: 6 }.into(),
                18 => Query::WindowSize { reply_code: 8 }.into(),
                22 => WindowCommand::PushTitle.into(),
                23 => WindowCommand::PopTitle.into(),
                8 => WindowCommand::ResizeTextArea {
                    rows: p(1, rows).max(1) as usize,
                    cols: p(2, cols).max(1) as usize,
                }
                .into(),
                _ => return None,
            },
            (b'h' | b'l', _) => {
                let on = final_byte == b'h';
                if private && p0 == 1048 {
                    // DEC 1048 — save/restore cursor as a mode.
                    return Some(if on {
                        Command::SaveCursor
                    } else {
                        Command::RestoreCursor
                    });
                }
                match params {
                    // Single mode (`?2026h`) — the common form, no Vec.
                    [code] => {
                        let mode = TermFlags::from_mode_code(*code, private);
                        if mode.is_empty() {
                            return None;
                        }
                        Command::SetMode { mode, on }
                    }
                    // Multi-mode (`?1006;1000h`, htop's ncurses mousemask).
                    codes => {
                        let modes = SmallVec::from_iter(
                            codes.iter().filter_map(|code| {
                                let m =
                                    TermFlags::from_mode_code(*code, private);
                                (!m.is_empty()).then_some(m)
                            }),
                        );
                        match modes.len() {
                            0 => return None,
                            1 => Command::SetMode { mode: modes[0], on },
                            _ => Command::SetModes { modes, on },
                        }
                    }
                }
            }
            // DECCARA (`CSI Pt;Pl;Pb;Pr;Ps… $ r`) — set the bold/underline/
            // blink/reverse attributes in a rectangle.
            (b'r', _) if dollar => {
                let (mask, value) = jetfish_protocol::rect_attr_mask(
                    params.get(4..).unwrap_or(&[]),
                    true,
                )?;
                Command::ChangeRectAttrs {
                    top: p(0, 1).max(1) as usize,
                    left: p(1, 1).max(1) as usize,
                    bottom: p(2, rows).max(1) as usize,
                    right: p(3, cols).max(1) as usize,
                    set: true,
                    mask,
                    value,
                }
            }
            // DECSTBM (1-based, inclusive) → 0-based. A missing or zero
            // bottom defaults to the full screen (xterm).
            (b'r', _) => {
                let top = p(0, 1).max(1) as usize - 1;
                let bottom = p(1, rows);
                let bottom =
                    if bottom <= 0 || bottom > rows { rows } else { bottom }
                        as usize
                        - 1;
                Command::SetScrollRegion { top, bottom }
            }
            (b'X', Some(b'?')) => Command::SelectiveEraseChars(n(0)), // DECSECH
            (b'X', _) => Command::EraseChars(n(0)),                   // ECH
            (b'P', Some(b'#')) => Command::PushColors, // XTPUSHCOLORS
            (b'Q', Some(b'#')) => Command::PopColors,  // XTPOPCOLORS
            (b'R', Some(b'#')) => Command::ReportColors, // XTPREPORTCOLORS
            (b'S', _) => Command::ScrollUp(n(0)),      // SU
            (b'T', _) => Command::ScrollDown(n(0)),    // SD
            (b'@', Some(b' ')) => Command::ScrollLeft(n(0)), // SL
            (b'@', _) => Command::InsertChars(n(0)),   // ICH
            (b'}', Some(b'\'')) => Command::InsertColumns(n(0)), // DECIC
            (b'~', Some(b'\'')) => Command::DeleteColumns(n(0)), // DECDC
            (b'P', _) => Command::DeleteChars(n(0)),   // DCH
            (b'L', _) => Command::InsertLines(n(0)),   // IL
            (b'M', _) => Command::DeleteLines(n(0)),   // DL
            (b'b', _) => Command::Repeat(n(0).min(MAX_REPEAT)), // REP
            (b'I', _) => Command::ForwardTab(n(0)),    // CHT
            (b'Z', _) => Command::BackwardTab(n(0)),   // CBT
            (b'g', _) => Command::TabClear(p0 as u8),  // TBC
            (b'W', Some(b'?')) if p0 == 5 => {
                Command::TabEvery8 // DECST8C (`CSI ?5W`)
            }
            (b'W', _) => Command::TabControl(p0 as u8), // CTC
            (b's', Some(b'?')) => Command::SaveModes(params.to_vec()), // XTSAVE
            // DECSLRM (`CSI Pl ; Pr s`) vs SCO save cursor (`CSI s`). A
            // param-less `CSI s` is ambiguous: SCOSC when DECLRMM (`?69`) is
            // off, DECSLRM (full-width reset) when on — the screens resolve
            // it in `AnsiSaveCursor`.
            (b's', _) => match params {
                [] => Command::AnsiSaveCursor,
                [pl] => Command::SetLeftRightMargins {
                    left: Some((*pl).max(1) as usize - 1),
                    right: None,
                },
                // `CSI ; Pr s` — empty left; `-2` is the empty-semicolon
                // sentinel.
                [-2, pr, ..] => Command::SetLeftRightMargins {
                    left: None,
                    right: Some((*pr).max(1) as usize - 1),
                },
                [pl, pr, ..] => Command::SetLeftRightMargins {
                    left: Some((*pl).max(1) as usize - 1),
                    right: Some((*pr).max(1) as usize - 1),
                },
            },
            (b'u', _) => Command::RestoreCursor, // ANSI restore cursor
            (b'p', Some(b'!')) => Command::SoftReset, // DECSTR
            // DECRQM: `CSI ? Ps $ p` (private) / `CSI Ps $ p` (ANSI).
            (b'p', _) if dollar => Query::Mode { param: p0, private }.into(),
            // DECCRA (`CSI Pt;Pl;Pb;Pr;Pp $ v`) — copy rectangular area.
            (b'v', _) if dollar => Command::CopyRect {
                top: p(0, 1).max(1) as usize,
                left: p(1, 1).max(1) as usize,
                bottom: p(2, rows).max(1) as usize,
                right: p(3, cols).max(1) as usize,
                page: p(4, 0),
            },
            // DECFRA (`CSI Pch;Pt;Pl;Pb;Pr $ x`) — fill rectangular area.
            (b'x', _) if dollar => Command::FillRect {
                ch: char::from_u32(p0.max(0) as u32).unwrap_or(' '),
                top: p(1, 1).max(1) as usize,
                left: p(2, 1).max(1) as usize,
                bottom: p(3, rows).max(1) as usize,
                right: p(4, cols).max(1) as usize,
            },
            // DECRQCRA (`CSI Pi;Pg;Pt;Pl;Pb;Pr * y`) — checksum a
            // rectangular area. The page number (param 1) is dropped: both
            // screens are single-page.
            (b'y', Some(b'*')) => Query::ChecksumRect {
                id: p0,
                top: p(2, 1).max(1) as usize,
                left: p(3, 1).max(1) as usize,
                bottom: p(4, rows).max(1) as usize,
                right: p(5, cols).max(1) as usize,
            }
            .into(),
            // DECERA (`CSI Pt;Pl;Pb;Pr $ z`) — erase rectangular area.
            (b'z', _) if dollar => Command::EraseRect {
                top: p(0, 1).max(1) as usize,
                left: p(1, 1).max(1) as usize,
                bottom: p(2, rows).max(1) as usize,
                right: p(3, cols).max(1) as usize,
            },
            _ => return None,
        };
        Some(cmd)
    }
}
