// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! DCS (`ESC P`) codec — the raw dispatch shape plus encode/decode.

use smallvec::SmallVec;

use vtty::{Event, TermFlags};

use super::codec::{DecodeSeq, EncodeSeq, Encoded};
use super::{Command, Query};

/// A raw DCS sequence; stateful and buffered, so it owns its payload rather
/// than borrowing.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Dcs {
    /// The DCS parameter values.
    pub params: SmallVec<i32, 8>,
    /// Bit `i` is set when parameter `i` was followed by a colon.
    pub sep: u16,
    /// The intermediate bytes (`$`, `+`, `=`, …).
    pub ints: Vec<u8>,
    /// The final byte (`q`, `s`, …).
    pub final_byte: u8,
    /// The buffered payload bytes (before `ESC \`).
    pub payload: Vec<u8>,
}

impl Dcs {
    /// Build a raw DCS sequence from its dispatch parts.
    pub const fn new(
        params: SmallVec<i32, 8>,
        sep: u16,
        ints: Vec<u8>,
        final_byte: u8,
        payload: Vec<u8>,
    ) -> Self {
        Self { params, sep, ints, final_byte, payload }
    }
}

impl DecodeSeq for Dcs {
    fn decode(self) -> Option<Command> {
        let cmd = match self.final_byte {
            // DECRQSS (`DCS $ q <setting> ST`).
            b'q' if self.ints.contains(&b'$') => {
                Command::Query(Query::Setting(self.payload))
            }
            // XTGETTCAP (`DCS + q <hex> ST`) — the payload is hex.
            b'q' if self.ints.contains(&b'+') => Command::Query(
                Query::Termcap(jetfish_protocol::hex_decode(&self.payload)),
            ),
            // Legacy iTerm2 synchronized updates (`DCS = 1 s` begin,
            // `DCS = 2 s` end).
            b's' if self.ints.contains(&b'=') => {
                let param = *self.params.first()?;
                if !matches!(param, 1 | 2) {
                    return None;
                }
                Command::SetMode {
                    mode: TermFlags::SYNC_OUTPUT,
                    on: param == 1,
                }
            }
            // Kitty graphics / Sixel — needs the stateful reassembly path,
            // not decodable to a single Command here.
            _ => return None,
        };
        Some(cmd)
    }
}

impl EncodeSeq for Dcs {
    fn encode(self) -> Encoded {
        // DCS params never carry colon separators in practice (the parser's
        // `hook` callback has no `sep`), so `Event::DcsHook` is a faithful
        // re-encoding.
        let mut intermediates = SmallVec::new();
        intermediates.extend_from_slice_copy(&self.ints);
        let mut out = Encoded::new();
        Event::DcsHook {
            params: self.params,
            intermediates,
            final_byte: self.final_byte,
        }
        .write_bytes(&mut out);
        out.extend_from_slice_copy(&self.payload);
        Event::DcsUnhook.write_bytes(&mut out);
        out
    }
}

#[cfg(test)]
#[path = "../../tests/command/dcs.rs"]
mod tests;
