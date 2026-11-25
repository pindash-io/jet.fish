// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! OSC 21, the Kitty Color Protocol: query/set/reset colors by key
//! (`foreground`, `background`, `cursor`, `selection_*`, or an ANSI index).

use bases::Base64;
use compact_str::{CompactString, format_compact};
use smallvec::SmallVec;

use vtty::ColorId;

use jetfish_protocol::{Defaults, Highlight, Palette};

/// A color key (`Pk` in `Pk = Pv`): the default colors, the cursor, the
/// selection, or an ANSI palette entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KittyColorKey {
    /// The dynamic/unset color — the empty `Pk=` value (reverse video, a
    /// cursor that takes the text color, or an unset ANSI entry).
    None,
    /// `foreground` — the default foreground text color.
    Foreground,
    /// `background` — the default background color.
    Background,
    /// `cursor` — the color of the text cursor.
    Cursor,
    /// `cursor_text` — the color of text under the cursor (not tracked).
    CursorText,
    /// `selection_background` — the selection background color.
    SelectionBackground,
    /// `selection_foreground` — the selection foreground color.
    SelectionForeground,
    /// A `0..=255` ANSI palette entry.
    Palette(usize),
}

impl KittyColorKey {
    /// The wire name for this key (the `Pk` in the OSC 21 reply). The
    /// palette entry is the bare index (no name); [`Self::None`] is the
    /// empty/dynamic value.
    pub const fn name(self) -> &'static str {
        match self {
            Self::None => "",
            Self::Foreground => "foreground",
            Self::Background => "background",
            Self::Cursor => "cursor",
            Self::CursorText => "cursor_text",
            Self::SelectionBackground => "selection_background",
            Self::SelectionForeground => "selection_foreground",
            Self::Palette(_) => "",
        }
    }
}

/// One `Pk = Pv` pair of an OSC 21 payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KittyColorOp {
    /// `Pk = ?` — query the current value.
    Query(KittyColorKey),
    /// `Pk = <color>` — set the value. `rgb: None` is the empty value: set
    /// the dynamic color ([`KittyColorKey::None`]).
    Set {
        /// The color key.
        key: KittyColorKey,
        /// The 0xRRGGBBAA color, or `None` for the dynamic/empty value.
        rgb: Option<u32>,
    },
    /// `Pk` (no `=`) — reset to the default.
    Reset(KittyColorKey),
    /// An unrecognized key — replied as `unknown=<base64>` (kitty).
    Unknown(String),
}

/// A resolved query-reply field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReplyEntry {
    /// `Pk = value` (the empty value for the dynamic color).
    Color(KittyColorKey, Option<u32>),
    /// `unknown = <base64>` (the raw unrecognized key name).
    Unknown(String),
}

impl ReplyEntry {
    /// Append this entry's `Pk = value` (or `unknown = <base64>`) wire form.
    fn append_to(&self, out: &mut CompactString) {
        match self {
            &Self::Color(key, rgb) => {
                out.push_str(&key_wire(key));
                out.push('=');
                if let Some(rgb) = rgb {
                    out.push_str(&rgb_wire(rgb));
                }
            }
            Self::Unknown(name) => {
                out.push_str("unknown=");
                out.push_str(&Base64::encode(name.as_bytes()));
            }
        }
    }
}

/// The wire form of a key: the name, or the bare index for a palette entry.
fn key_wire(key: KittyColorKey) -> CompactString {
    match key {
        KittyColorKey::Palette(index) => format_compact!("{index}"),
        key => CompactString::from(key.name()),
    }
}

/// The `rgb:rrrr/gggg/bbbb` wire form of a color.
fn rgb_wire(rgb: u32) -> CompactString {
    let [r, g, b] = vtty::u32_to_rgb(rgb);
    format_compact!("rgb:{r:02x}{r:02x}/{g:02x}{g:02x}/{b:02x}{b:02x}")
}

/// Decode the `Pk = Pv ; …` payload into ops. Unrecognized keys become
/// [`KittyColorOp::Unknown`] (replied as `unknown=`). `None` when no op
/// decodes.
pub(crate) fn parse(pairs: &[&[u8]]) -> Option<Vec<KittyColorOp>> {
    let ops: Vec<_> =
        pairs.iter().filter_map(|&pair| parse_pair(pair)).collect();
    (!ops.is_empty()).then_some(ops)
}

/// Decode one `Pk = Pv` pair; `None` skips an invalid color value.
fn parse_pair(pair: &[u8]) -> Option<KittyColorOp> {
    // Split on the first `=`; a pair without one is a bare key (reset).
    let (key, value) = match pair.iter().position(|&b| b == b'=') {
        Some(i) => (&pair[..i], Some(&pair[i + 1..])),
        None => (pair, None),
    };
    let Some(key) = key_of(key) else {
        // An unrecognized key: keep it (queries reply `unknown=`).
        return Some(KittyColorOp::Unknown(
            String::from_utf8_lossy(key).into_owned(),
        ));
    };
    Some(match value {
        Some(b"?") => KittyColorOp::Query(key),
        Some(b"") => KittyColorOp::Set { key, rgb: None },
        Some(v) => KittyColorOp::Set { key, rgb: Some(vtty::parse(v)?) },
        None => KittyColorOp::Reset(key),
    })
}

/// Map an OSC 21 key name (or a bare `0..=255` index) to a [`KittyColorKey`].
fn key_of(bytes: &[u8]) -> Option<KittyColorKey> {
    Some(match bytes {
        b"foreground" => KittyColorKey::Foreground,
        b"background" => KittyColorKey::Background,
        b"cursor" => KittyColorKey::Cursor,
        b"cursor_text" => KittyColorKey::CursorText,
        b"selection_background" => KittyColorKey::SelectionBackground,
        b"selection_foreground" => KittyColorKey::SelectionForeground,
        digits => KittyColorKey::Palette(ansi_index(digits)?),
    })
}

/// Parse a bare `0..=255` ANSI index (all decimal digits, capped at 255).
fn ansi_index(bytes: &[u8]) -> Option<usize> {
    let mut index = 0_usize;
    for &b in bytes {
        let d = b.checked_sub(b'0').filter(|&d| d <= 9)?;
        index = index * 10 + d as usize;
        if index > 255 {
            return None;
        }
    }
    Some(index)
}

/// Apply the full OSC 21 payload to the shared color state: mutate the
/// SET/RESET ops, resolve the QUERY ops, and return the reply (if any).
#[must_use]
pub fn apply(
    palette: &mut Palette,
    defaults: &mut Defaults<u32>,
    highlight: &mut Highlight<u32>,
    ops: &[KittyColorOp],
) -> Option<CompactString> {
    for op in ops {
        match op {
            KittyColorOp::Query(_) | KittyColorOp::Unknown(_) => {}
            KittyColorOp::Set { key, rgb: None } => match key {
                // Dynamic: cursor takes the text color, selection reverses.
                KittyColorKey::Cursor => defaults.cursor = defaults.fg,
                KittyColorKey::SelectionBackground => highlight.bg = None,
                KittyColorKey::SelectionForeground => highlight.fg = None,
                // `foreground`/`background` have no dynamic value (kitty:
                // "not applicable"); `cursor_text` and ANSI entries are not
                // tracked / not allowed to be dynamic.
                _ => {}
            },
            &KittyColorOp::Set { key, rgb: Some(rgb) } => match key {
                KittyColorKey::Foreground => {
                    palette.set_default_fg(Some(rgb));
                }
                KittyColorKey::Background => {
                    palette.set_default_bg(Some(rgb));
                }
                KittyColorKey::Cursor => defaults.cursor = rgb,
                KittyColorKey::CursorText => {}
                KittyColorKey::SelectionBackground => {
                    highlight.bg = Some(rgb);
                }
                KittyColorKey::SelectionForeground => {
                    highlight.fg = Some(rgb);
                }
                KittyColorKey::Palette(index) => {
                    palette.set_entry(index, rgb);
                }
                KittyColorKey::None => {}
            },
            KittyColorOp::Reset(key) => match key {
                KittyColorKey::Foreground => palette.set_default_fg(None),
                KittyColorKey::Background => palette.set_default_bg(None),
                KittyColorKey::Cursor => defaults.cursor = defaults.fg,
                KittyColorKey::CursorText => {}
                KittyColorKey::SelectionBackground => highlight.bg = None,
                KittyColorKey::SelectionForeground => highlight.fg = None,
                &KittyColorKey::Palette(index) => palette.reset_entry(index),
                KittyColorKey::None => {}
            },
        }
    }
    let entries: Vec<ReplyEntry> = ops
        .iter()
        .filter_map(|op| match op {
            &KittyColorOp::Query(key) => Some(ReplyEntry::Color(
                key,
                resolve(key, defaults, highlight, palette),
            )),
            KittyColorOp::Unknown(name) => {
                Some(ReplyEntry::Unknown(name.clone()))
            }
            _ => None,
        })
        .collect();
    reply(&entries)
}

/// Resolve a queried key to its current value: `Some(rgb)` for a concrete
/// color, `None` for the dynamic/unset color (replied as the empty `Pk=`).
pub(crate) fn resolve(
    key: KittyColorKey,
    defaults: &Defaults<u32>,
    highlight: &Highlight<u32>,
    palette: &Palette,
) -> Option<u32> {
    let rgb = match key {
        KittyColorKey::None => return None,
        // The effective default: the OSC 10/11 override, else the theme
        // default.
        KittyColorKey::Foreground => {
            palette.default_fg().unwrap_or(defaults.fg)
        }
        KittyColorKey::Background => {
            palette.default_bg().unwrap_or(defaults.bg)
        }
        KittyColorKey::Cursor => defaults.cursor,
        // Not tracked — dynamic (takes the text color).
        KittyColorKey::CursorText => return None,
        KittyColorKey::SelectionBackground => highlight.bg?,
        KittyColorKey::SelectionForeground => highlight.fg?,
        KittyColorKey::Palette(index) => palette.get(ColorId::from(index))?,
    };
    Some(rgb)
}

/// Format an OSC 21 reply (`ESC]21;Pk=value;…ESC\\`) for the resolved fields.
/// A dynamic/unset color is the empty `Pk=`; an unknown key is
/// `unknown=<base64>`.
pub(crate) fn reply(entries: &[ReplyEntry]) -> Option<CompactString> {
    if entries.is_empty() {
        return None;
    }
    let mut out = CompactString::from("\x1b]21;");
    for (i, entry) in entries.iter().enumerate() {
        if i > 0 {
            out.push(';');
        }
        entry.append_to(&mut out);
    }
    out.push_str("\x1b\\");
    Some(out)
}

/// Encode the ops back into the `OSC 21 ; Pk = Pv ; … ST` wire bytes.
pub(crate) fn encode(ops: &[KittyColorOp]) -> SmallVec<u8, 8> {
    let mut out = SmallVec::with_capacity(16);
    out.extend_from_slice_copy(b"\x1b]21");
    for op in ops {
        out.push(b';');
        match op {
            KittyColorOp::Unknown(name) => {
                out.extend_from_slice_copy(name.as_bytes());
                out.extend_from_slice_copy(b"=?");
            }
            &KittyColorOp::Query(key) => {
                out.extend_from_slice_copy(key_wire(key).as_bytes());
                out.extend_from_slice_copy(b"=?");
            }
            &KittyColorOp::Set { key, rgb } => {
                out.extend_from_slice_copy(key_wire(key).as_bytes());
                out.push(b'=');
                if let Some(rgb) = rgb {
                    out.extend_from_slice_copy(rgb_wire(rgb).as_bytes());
                }
            }
            &KittyColorOp::Reset(key) => {
                out.extend_from_slice_copy(key_wire(key).as_bytes());
            }
        }
    }
    out.extend_from_slice_copy(b"\x1b\\");
    out
}

#[cfg(test)]
#[path = "../../tests/command/kitty_color.rs"]
mod tests;
