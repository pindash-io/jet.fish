// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! OSC (`ESC ]`) decoder.

use compact_str::CompactString;
use smallvec::SmallVec;

use vtty::Event;

use jetfish_protocol::Progress;
use jetfish_protocol::percent_decode;

pub use jetfish_protocol::cwd_path;

use super::codec::{DecodeSeq, EncodeSeq, Encoded};
use super::{
    ColorCommand, Command, DefaultColor, Hyperlink, OscCommand, Query,
    ShellPrompt,
};

/// Cap on the OSC 7 working-directory payload.
const MAX_CWD_LEN: usize = 1024;

/// OSC 52 clipboard access policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Osc52 {
    /// Ignore OSC 52 entirely.
    Disabled,
    /// Accept the copy sequence, reject the read query (the secure default).
    #[default]
    OnlyCopy,
    /// Accept the read query, reject the copy sequence.
    OnlyPaste,
    /// Accept both copy and query.
    CopyPaste,
}

impl Osc52 {
    /// Whether the policy permits the OSC 52 copy (clipboard write) form.
    #[must_use]
    pub const fn allows_copy(self) -> bool {
        matches!(self, Self::OnlyCopy | Self::CopyPaste)
    }

    /// Whether the policy permits the OSC 52 query (clipboard read) form.
    #[must_use]
    pub const fn allows_paste(self) -> bool {
        matches!(self, Self::OnlyPaste | Self::CopyPaste)
    }
}

/// A raw OSC dispatch (`ESC ] …`).
#[derive(Debug)]
pub struct Osc<'a> {
    /// The `;`-split OSC parameters.
    pub params: &'a [&'a [u8]],
    /// The clipboard access policy gating OSC 52.
    pub osc52: Osc52,
}

impl<'a> Osc<'a> {
    /// A raw OSC dispatch with the default clipboard policy — for re-encoding
    /// and other call sites where the policy is not part of the wire sequence.
    #[must_use]
    pub fn new(params: &'a [&'a [u8]]) -> Self {
        Self { params, osc52: Osc52::default() }
    }
}

impl EncodeSeq for Osc<'_> {
    fn encode(self) -> Encoded {
        // The `osc52` policy is screen config, not part of the wire sequence.
        let mut out = Encoded::new();
        Event::OscDispatch {
            params: SmallVec::from_iter(
                self.params.iter().map(|p| p.to_vec()),
            ),
            bell_terminated: false,
        }
        .write_bytes(&mut out);
        out
    }
}

impl DecodeSeq for Osc<'_> {
    fn decode(self) -> Option<Command> {
        Self::parse(self.params, self.osc52)
    }
}

impl Osc<'_> {
    /// Decode an OSC (`ESC ]`) sequence into a [`Command`], or `None` for
    /// unhandled sequences. `osc52` gates the clipboard read/write.
    #[must_use]
    pub fn parse(params: &[&[u8]], osc52: Osc52) -> Option<Command> {
        let num = |i: usize| {
            params
                .get(i)
                .and_then(|v| CompactString::from_utf8(v).ok())
                .and_then(|s| s.parse::<usize>().ok())
        };
        let cmd = match params.first().copied()? {
            b"0" | b"1" | b"2" => OscCommand::SetTitle(
                CompactString::from_utf8_lossy(params.get(1)?),
            )
            .into(),
            // Handled (xterm ctlseqs): 4/104 palette, 10-12/110-112 default
            // colors, 17/19/117/119 selection highlight.
            //
            // Deliberately ignored (unsupported — no reply, no state): 5/105
            // DECPCTERM special colors (jetfish has no per-attribute color
            // override), 6 undocumented, 13-16/113-116 pointer + Tektronix
            // colors (drawn by the OS / no Tektronix emulation), 18/118.
            b"4" => {
                if params.get(2).copied()? == b"?" {
                    return Some(Query::PaletteColor(num(1)?).into());
                }
                let entries: Vec<(usize, u32)> = (1..params.len() - 1)
                    .step_by(2)
                    .filter_map(|i| {
                        Some((num(i)?, vtty::osc_to_rgba_u32(params[i + 1])?))
                    })
                    .collect();
                if entries.is_empty() {
                    return None;
                }
                ColorCommand::SetPaletteBatch(entries).into()
            }
            b"7" => {
                let v = params.get(1)?;
                if v.len() > MAX_CWD_LEN {
                    return None;
                }
                OscCommand::SetCwd(CompactString::from_utf8_lossy(v)).into()
            }
            // OSC 8 — hyperlink: the URI is everything after the second `;` and
            // can itself contain `;`. Empty → close the link.
            b"8" => {
                // OSC 8;params;uri. `params` (may be empty) is the `;`
                // segment after `8` holding colon-separated `key=value`
                // pairs — only `id=` matters. The URI is everything after
                // the params segment (it may itself contain `;`).
                let mut id: Option<CompactString> = None;
                let uri = match params.len() {
                    2 if !params[1].is_empty() => {
                        Some(CompactString::from_utf8_lossy(params[1]))
                    }
                    n if n > 2 => {
                        for kv in params[1].split(|&b| b == b':') {
                            // `id=` with a non-empty value sets the id;
                            // an empty value (`id=`) is treated as no id.
                            if let Some(v) = kv.strip_prefix(b"id=")
                                && !v.is_empty()
                            {
                                id = Some(CompactString::from_utf8_lossy(v));
                            }
                        }
                        let uri = params[2..].join(&b';');
                        if uri.is_empty() {
                            None
                        } else {
                            Some(CompactString::from_utf8_lossy(&uri))
                        }
                    }
                    _ => None,
                };
                // An empty URI with an `id=` is invalid;
                // only a bare `OSC 8;;` closes the link.
                if uri.is_none() && id.is_some() {
                    return None;
                }
                OscCommand::SetHyperlink(uri.map(|uri| Hyperlink { uri, id }))
                    .into()
            }
            b"9" => match params.get(1).copied()? {
                b"3" => {
                    // ConEmu tab title: `OSC 9;3` resets, `OSC 9;3;<title>` sets.
                    let title = params
                        .get(2)
                        .map(|v| CompactString::from_utf8_lossy(v));
                    OscCommand::SetTabTitle(title).into()
                }
                b"4" => match params.get(2).copied()? {
                    b"0" => {
                        OscCommand::Progress(Progress::Indeterminate).into()
                    }
                    b"1" => {
                        let percent = params
                            .get(3)
                            .and_then(|v| CompactString::from_utf8(v).ok())
                            .and_then(|s| s.parse::<u8>().ok())
                            .unwrap_or(0)
                            .min(100);
                        OscCommand::Progress(Progress::Percent(percent)).into()
                    }
                    b"2" => OscCommand::Progress(Progress::Done).into(),
                    b"3" => OscCommand::Progress(Progress::Error).into(),
                    _ => return None,
                },
                // ConEmu uses `OSC 9;<digit command>[;<data>]` (1/2/5/6/10/11/12);
                // those are ignored. Only a free-form second parameter is an
                // iTerm2 notification (`OSC 9;<message>`).
                msg if !msg.is_empty()
                    && !msg.iter().all(u8::is_ascii_digit) =>
                {
                    OscCommand::Notify(CompactString::from_utf8_lossy(msg))
                        .into()
                }
                _ => return None,
            },
            // xterm OSC color codes: 10/11/12 set the default fg/bg/cursor
            // (`?` queries), 110/111/112 reset them, 17/19 set the selection
            // highlight bg/fg, 117/119 reset it.
            code if let Some(color) = osc_color(code) => match color {
                OscColor::SetDefault(target) => {
                    if params.get(1)? == b"?" {
                        Query::DefaultColor(target).into()
                    } else {
                        let rgb = params
                            .get(1)
                            .and_then(|v| vtty::osc_to_rgba_u32(v))?;
                        ColorCommand::SetDefault { target, rgb }.into()
                    }
                }
                OscColor::ResetDefault(target) => {
                    ColorCommand::ResetDefault(target).into()
                }
                OscColor::SetHighlight(target) => {
                    let rgb = params
                        .get(1)
                        .and_then(|v| vtty::osc_to_rgba_u32(v))?;
                    ColorCommand::SetHighlight { target, rgb }.into()
                }
                OscColor::ResetHighlight(target) => {
                    ColorCommand::ResetHighlight(target).into()
                }
            },
            // OSC 21 — Kitty Color Protocol: `Pk = Pv ; …` pairs query/set/
            // reset colors by key (`foreground`, `background`, `cursor`,
            // `selection_*`, or a `0..=255` ANSI index).
            b"21" => OscCommand::KittyColor(super::kitty_color::parse(
                &params[1..],
            )?)
            .into(),
            b"22" => {
                let name = params
                    .get(1)
                    .map(|v| CompactString::from_utf8_lossy(v))?;
                OscCommand::MouseCursor(name).into()
            }
            b"50" => {
                let digit = params
                    .get(1)
                    .copied()
                    .unwrap_or_default()
                    .strip_prefix(b"CursorShape=")
                    .and_then(|v| v.first())
                    .copied()?;
                if !matches!(digit, b'0'..=b'2') {
                    return None;
                }
                OscCommand::CursorShape((digit - b'0') as i32).into()
            }
            b"52" => {
                // OSC 52;<selector>;<base64-or-?>. Validate the selector like
                // alacritty (only `c`/`p`/`s`), then gate copy/query on the
                // access policy.
                let selector =
                    params.get(1).and_then(|v| v.first()).copied()?;
                if !matches!(selector, b'c' | b'p' | b's') {
                    return None;
                }
                let payload = params.get(2).copied().unwrap_or(b"");
                if payload == b"?" {
                    if osc52.allows_paste() {
                        Query::Clipboard(selector).into()
                    } else {
                        return None;
                    }
                } else if osc52.allows_copy() {
                    OscCommand::SetClipboard(CompactString::from_utf8_lossy(
                        payload,
                    ))
                    .into()
                } else {
                    return None;
                }
            }
            // OSC 99 (kitty) — desktop notifications.
            b"99" => OscCommand::Notification(
                crate::notification::parse_osc99(params)?,
            )
            .into(),
            b"104" => ColorCommand::ResetPalette(num(1)).into(),
            // OSC 113/114 (mouse fg/bg) and 115/116 (tektronix fg/bg) reset
            // colors jetfish does not track — ignored.
            // OSC 133 — shell integration marker.
            b"133" => {
                OscCommand::ShellIntegration(parse_shell_prompt(params)?)
                    .into()
            }
            // OSC 777 (urxvt `notify`) — desktop notifications.
            b"777" => OscCommand::Notification(
                crate::notification::parse_osc777(params)?,
            )
            .into(),
            #[cfg(feature = "iterm2")]
            b"1337" => {
                Command::Iterm2Image(jetfish_image::parse_iterm2(params)?)
            }
            _ => return None,
        };
        Some(cmd)
    }
}

/// The xterm OSC color codes: 10/11/12 set the default fg/bg/cursor,
/// 110/111/112 reset them, 17/19 set the selection highlight bg/fg, and
/// 117/119 reset it.
enum OscColor {
    SetDefault(DefaultColor),
    ResetDefault(DefaultColor),
    SetHighlight(DefaultColor),
    ResetHighlight(DefaultColor),
}

const fn osc_color(code: &[u8]) -> Option<OscColor> {
    let color = match code {
        b"10" => OscColor::SetDefault(DefaultColor::Foreground),
        b"11" => OscColor::SetDefault(DefaultColor::Background),
        b"12" => OscColor::SetDefault(DefaultColor::Cursor),
        b"110" => OscColor::ResetDefault(DefaultColor::Foreground),
        b"111" => OscColor::ResetDefault(DefaultColor::Background),
        b"112" => OscColor::ResetDefault(DefaultColor::Cursor),
        b"17" => OscColor::SetHighlight(DefaultColor::Background),
        b"19" => OscColor::SetHighlight(DefaultColor::Foreground),
        b"117" => OscColor::ResetHighlight(DefaultColor::Background),
        b"119" => OscColor::ResetHighlight(DefaultColor::Foreground),
        _ => return None,
    };
    Some(color)
}

/// Decode the OSC 133 options: `aid=`, `cmdline_url=`, and a bare exit-code
/// number.
fn parse_shell_prompt(params: &[&[u8]]) -> Option<ShellPrompt> {
    let action = params.get(1).and_then(|v| v.first()).copied()?;
    let mut aid = None;
    let mut exit_code = None;
    let mut cmdline = None;
    for opt in &params[2..] {
        if let Some(v) = opt.strip_prefix(b"aid=") {
            aid = Some(CompactString::from_utf8_lossy(v));
        } else if let Some(v) = opt.strip_prefix(b"cmdline_url=") {
            cmdline = Some(percent_decode(v));
        } else if exit_code.is_none()
            && let Ok(str) = CompactString::from_utf8(opt)
            && let Ok(code) = str.parse::<i32>()
        {
            exit_code = Some(code);
        }
    }
    Some(ShellPrompt { action, aid, exit_code, cmdline })
}
