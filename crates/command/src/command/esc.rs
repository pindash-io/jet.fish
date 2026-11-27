// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! ESC (`ESC x`) decoder.

use smallvec::SmallVec;

use vtty::{Charset, Event};

use super::codec::{DecodeSeq, EncodeSeq, Encoded};
use super::{Command, Marker, Move, Query};

/// A raw ESC dispatch (`ESC x`).
#[derive(Debug)]
pub struct Esc<'a> {
    /// The intermediate bytes (`(`, `)`, `*`, `+`, `#`).
    pub ints: &'a [u8],
    /// The final byte.
    pub byte: u8,
}

impl EncodeSeq for Esc<'_> {
    fn encode(self) -> Encoded {
        let mut intermediates = SmallVec::new();
        intermediates.extend_from_slice_copy(self.ints);
        let mut out = Encoded::new();
        Event::EscDispatch { intermediates, byte: self.byte }
            .write_bytes(&mut out);
        out
    }
}

impl DecodeSeq for Esc<'_> {
    fn decode(self) -> Option<Command> {
        Self::parse(self.ints, self.byte)
    }
}

impl Esc<'_> {
    /// Decode an ESC (`ESC x`) sequence into a [`Command`], or `None` for
    /// unhandled sequences.
    #[must_use]
    pub fn parse(ints: &[u8], byte: u8) -> Option<Command> {
        if let Some(&marker) = ints.first() {
            if let Some(marker) = Marker::parse(marker) {
                let charset = Charset::from(byte);
                let g = marker.index();
                return Some(Command::CharsetDesignate { g, charset });
            }
            // DECALN (`ESC # 8`).
            if marker == b'#' && byte == b'8' {
                return Some(Command::AlignmentTest);
            }
        }
        let cmd = match byte {
            b'D' => Move::Index.into(),                    // IND
            b'M' => Move::ReverseIndex.into(),             // RI
            b'E' => Move::NewLine.into(),                  // NEL
            b'H' => Command::TabSet,                       // HTS
            b'7' => Command::SaveCursor,                   // DECSC
            b'8' => Command::RestoreCursor,                // DECRC
            b'=' => Command::SetKeypad(true),              // DECKPAM
            b'>' => Command::SetKeypad(false),             // DECPNM
            b'c' => Command::Reset,                        // RIS
            b'Z' => Command::Query(Query::DeviceAttrs(1)), // DECID = DA1
            b'n' => Command::LockingShift(2),              // LS2
            b'o' => Command::LockingShift(3),              // LS3
            _ => return None,
        };
        Some(cmd)
    }
}
