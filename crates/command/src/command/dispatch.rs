// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The `Perform` → [`Apply`] dispatch shims. CSI/ESC/OSC are screen-agnostic;
//! print and DCS stay screen-specific.

use jetfish_protocol::CursorPos;

use crate::command::{Command, Csi, CsiCtx, Esc, Osc, Osc52, Query};
use crate::commands::Apply;

/// One escape-sequence family's worth of the [`vtty::Perform`] surface.
pub trait Handler<S: Apply + ?Sized> {
    /// A printable character.
    fn print(&mut self, _s: &mut S, _c: char) {}

    /// A run of printable bytes.
    fn print_bytes(&mut self, _s: &mut S, _bytes: &[u8]) {}

    /// A C0 control byte.
    fn execute(&mut self, _s: &mut S, _b: u8) {}

    /// A CSI sequence (`ESC [ … final`).
    fn csi(
        &mut self,
        _s: &mut S,
        _params: &[i32],
        _sep: u16,
        _ints: &[u8],
        _final_byte: u8,
    ) {
    }

    /// An ESC sequence (`ESC … final`).
    fn esc(&mut self, _s: &mut S, _ints: &[u8], _byte: u8) {}

    /// A DCS sequence began (`ESC P …`).
    fn dcs_hook(
        &mut self,
        _s: &mut S,
        _params: &[i32],
        _ints: &[u8],
        _final_byte: u8,
    ) {
    }

    /// One DCS payload byte.
    fn dcs_put(&mut self, _s: &mut S, _byte: u8) {}

    /// A DCS sequence ended (`ST`).
    fn dcs_unhook(&mut self, _s: &mut S) {}

    /// An OSC sequence ended (`ESC ] … BEL`/`ST`).
    fn osc(&mut self, _s: &mut S, _params: &[&[u8]], _bell: bool) {}
}

/// CSI (`ESC [`) — flush buffered text, decode, apply.
#[derive(Debug)]
pub struct CsiHandler;

impl<S: Apply + ?Sized> Handler<S> for CsiHandler {
    fn csi(
        &mut self,
        s: &mut S,
        params: &[i32],
        sep: u16,
        ints: &[u8],
        final_byte: u8,
    ) {
        s.flush();
        dispatch_csi(s, params, sep, ints, final_byte, s.decode_ctx());
    }
}

/// ESC (`ESC x`) — decode, apply.
#[derive(Debug)]
pub struct EscHandler;

impl<S: Apply + ?Sized> Handler<S> for EscHandler {
    fn esc(&mut self, s: &mut S, ints: &[u8], byte: u8) {
        // Flush buffered text first (the cell screen batches printable runs).
        s.flush();
        dispatch_esc(s, ints, byte);
    }
}

/// OSC (`ESC ]`) — flush buffered text, decode, apply.
#[derive(Debug)]
pub struct OscHandler;

impl<S: Apply + ?Sized> Handler<S> for OscHandler {
    fn osc(&mut self, s: &mut S, params: &[&[u8]], _bell: bool) {
        s.flush();
        dispatch_osc(s, params, s.osc_policy());
    }
}

/// Decode + apply an OSC (`ESC ]`) command with the given clipboard policy.
#[inline]
pub fn dispatch_osc<S: Apply + ?Sized>(
    s: &mut S,
    params: &[&[u8]],
    osc52: Osc52,
) {
    let Some(cmd) = Osc::parse(params, osc52) else {
        return;
    };
    s.apply(cmd);
}

/// Decode + apply an ESC (`ESC x`) command.
#[inline]
pub fn dispatch_esc<S: Apply + ?Sized>(s: &mut S, ints: &[u8], byte: u8) {
    let Some(cmd) = Esc::parse(ints, byte) else {
        return;
    };
    s.apply(cmd);
}

/// Decode + apply a CSI (`ESC [`) command against the given [`CsiCtx`].
#[inline]
pub fn dispatch_csi<S: Apply + ?Sized>(
    s: &mut S,
    params: &[i32],
    sep: u16,
    ints: &[u8],
    final_byte: u8,
    ctx: CsiCtx,
) {
    // `>4m` / `?4m` are the only `m` forms that need the full parse.
    let special = matches!(ints.first(), Some(b'>' | b'?'))
        && params.first().copied().unwrap_or(0) == 4;

    match final_byte {
        // Direct SGR: plain `m` dominates full-screen TUIs.
        b'm' if !special => {
            s.apply_sgr(params, sep);
            return;
        }
        // Direct CUP: `CSI r ; c H` / `f` — skip the `Command::Move`
        // round-trip; the DECOM normalization matches `Csi::parse`.
        b'H' | b'f' => {
            let mut pos = CursorPos::new(
                params.get(1).copied().unwrap_or(1).max(1) as usize - 1,
                params.first().copied().unwrap_or(1).max(1) as usize - 1,
            );
            if ctx.decom {
                pos = (pos + CursorPos::new(ctx.margin.x, ctx.scroll.x))
                    .min(CursorPos::new(ctx.margin.y, ctx.scroll.y));
            }
            s.move_to(pos);
            return;
        }
        _ => {}
    }
    let Some(cmd) = Csi::parse(params, sep, ints, final_byte, ctx) else {
        return;
    };
    s.apply(cmd);
}

/// Whether a DCS/APC hook is a DECRQSS (`DCS $ q`) / XTGETTCAP (`DCS + q`)
/// query, returning `Some(hex)` for the `+ q` (hex-payload) form.
#[must_use]
#[inline]
pub fn dcs_query_kind(ints: &[u8], final_byte: u8) -> Option<bool> {
    (final_byte == b'q' && (ints.contains(&b'$') || ints.contains(&b'+')))
        .then(|| ints.contains(&b'+'))
}

/// Decode a buffered DECRQSS / XTGETTCAP payload and apply the query.
#[inline]
pub fn apply_dcs_query<S: Apply + ?Sized>(
    s: &mut S,
    payload: Vec<u8>,
    hex: bool,
) {
    let query = if hex {
        Query::Termcap(jetfish_protocol::hex_decode(&payload))
    } else {
        Query::Setting(payload)
    };
    s.apply(Command::Query(query));
}
