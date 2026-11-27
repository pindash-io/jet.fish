// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! iTerm2 inline images protocol (`OSC 1337;File=…`).
//!
//! ```notrust
//! ESC ] 1337 ; File=name=…;size=…;width=…;height=…;inline=1 : <base64> BEL/ST
//! ```

use std::sync::Arc;

use bases::Base64;
use compact_str::CompactString;
use glam::{USizeVec2, UVec2};

use crate::{Image, ImageCtx, ImageKind};

/// A parsed iTerm2 inline image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Iterm2Image {
    /// The `name=` argument (a filename/key, informational).
    pub name: Option<CompactString>,
    /// Display width (`width=`), if any.
    pub width: Option<Resize>,
    /// Display height (`height=`), if any.
    pub height: Option<Resize>,
    /// Whether to preserve the aspect ratio (`preserveAspectRatio=1`, the
    /// default). `preserveAspectRatio=0` stretches.
    pub preserve_aspect_ratio: bool,
    /// `WezTerm` extension: `doNotMoveCursor=1` leaves the cursor in place (TUIs
    /// on the bottom row); otherwise it moves right of the image.
    pub do_not_move_cursor: bool,
    /// Decoded file bytes (base64 → PNG/raw).
    pub bytes: Vec<u8>,
}

/// An iTerm2 image size argument (`width=`/`height=`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resize {
    /// A number of character cells (no unit).
    Cells(u32),
    /// A number of pixels (`Npx`).
    Pixels(u32),
    /// A percentage of the window (`N%`).
    Percent(u32),
    /// Computed from the original image size (`auto` or omitted).
    Auto,
}

/// Parse an `OSC 1337;File=…` parameter list; `None` when not `inline=1` or
/// the base64 is invalid.
#[must_use]
pub fn parse_iterm2(params: &[&[u8]]) -> Option<Iterm2Image> {
    if !matches!(params.first()?, &b"1337") {
        return None;
    }

    let body = &params[1..];

    // Skip the key-value parse when no `inline=` key is present: the probe
    // shape (name/size only) carries no image.
    if !body
        .iter()
        .any(|p| p.strip_prefix(b"File=").unwrap_or(p).starts_with(b"inline="))
    {
        return None;
    }

    let mut name: Option<CompactString> = None;
    let mut width = None;
    let mut height = None;
    let mut preserve_aspect_ratio = true;
    let mut do_not_move_cursor = false;
    let mut inline = false;
    let mut contents: Option<&[u8]> = None;

    for (i, raw) in body.iter().enumerate() {
        let mut param = *raw;
        // The first body param carries the `File=` prefix.
        if i == 0 {
            param = param.strip_prefix(b"File=")?;
        }
        let mut pair = param.splitn(2, |&b| b == b'=');
        let key = pair.next()?;
        let mut value = pair.next()?;

        // The last param carries the base64 after the first ':'.
        if i == body.len() - 1 {
            let mut pair = value.splitn(2, |&b| b == b':');
            value = pair.next()?;
            contents = pair.next();
        }

        match key {
            // Defer the copy: the no-inline case returns below without
            // ever using it.
            b"name" => name = CompactString::from_utf8(value).ok(),
            b"width" => width = parse_resize(value),
            b"height" => height = parse_resize(value),
            b"preserveAspectRatio" => preserve_aspect_ratio = value != b"0",
            b"doNotMoveCursor" => do_not_move_cursor = value == b"1",
            b"inline" => inline = value == b"1",
            _ => {}
        }
    }

    if !inline {
        return None;
    }
    let contents = contents?;
    let bytes = Base64::decode(contents).ok()?;

    Some(Iterm2Image {
        name,
        width,
        height,
        preserve_aspect_ratio,
        do_not_move_cursor,
        bytes,
    })
}

/// Parse an iTerm2 `width=`/`height=` value: `N` cells, `Npx` pixels,
/// `N%` window percent, or `auto`/empty for auto.
fn parse_resize(value: &[u8]) -> Option<Resize> {
    if value.is_empty() || value == b"auto" {
        return Some(Resize::Auto);
    }
    let first_nondigit = value.iter().position(|b| !b.is_ascii_digit());
    let Some(pos) = first_nondigit else {
        return CompactString::from_utf8(value)
            .ok()?
            .parse()
            .ok()
            .map(Resize::Cells);
    };
    let (num, unit) = value.split_at(pos);
    let n: u32 = CompactString::from_utf8(num).ok()?.parse().ok()?;
    let resize = match unit {
        b"%" => Resize::Percent(n),
        b"px" => Resize::Pixels(n),
        _ => return None,
    };
    Some(resize)
}

/// Decode an iTerm2 inline image payload to `(width, height, RGBA8)` with the
/// configured backend: `simple` = PNG, else the `image` crate's features.
#[must_use]
pub(crate) fn decode_iterm2(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    crate::decode_image(bytes)
}

/// Decode and lay out an iTerm2 inline image at the cursor.
#[must_use]
pub fn place_iterm2(img: &Iterm2Image, ctx: ImageCtx) -> Option<Image> {
    let (w, h, rgba) = decode_iterm2(&img.bytes)?;
    Some(Image {
        kind: ImageKind::Kitty,
        id: 0,
        version: crate::parser::next_version(),
        anchor: USizeVec2::new(
            ctx.cursor_col.saturating_sub(1),
            ctx.screen_start + ctx.cursor_row.saturating_sub(1),
        ),
        cell_size: UVec2::ZERO,
        size: UVec2::new(w, h),
        cell_offset: UVec2::ZERO,
        z_index: 0,
        aspect: UVec2::ONE,
        pixels: Arc::from(rgba),
    })
}

#[cfg(test)]
#[path = "../tests/iterm2/mod.rs"]
mod tests;
