// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! VT image protocols: kitty graphics (`DCS q` / `APC _G`), DEC Sixel, and
//! iTerm2 inline images (`OSC 1337;File=…`). Parsing/decoding only — the host
//! owns rasterization + placement.

#![expect(
    clippy::cast_possible_truncation,
    reason = "Terminal cell/pixel math truncates widths and coordinates to fixed-size types where the range is known-safe"
)]
#![expect(
    missing_debug_implementations,
    reason = "Terminal state structs are large and Debug is not needed"
)]
#![deny(missing_docs)]

#[cfg(feature = "iterm2")]
mod iterm2;
#[cfg(feature = "kitty")]
mod kitty;
mod parser;
#[cfg(all(
    not(feature = "simple"),
    any(feature = "png", feature = "jpeg", feature = "gif", feature = "webp")
))]
mod resize;
#[cfg(feature = "simple")]
mod simple;
#[cfg(feature = "sixel")]
mod sixel;

/// Maximum kitty-graphics/Sixel payload (64 MiB), bounding an unbounded image
/// payload.
pub const MAX_IMAGE_PAYLOAD: usize = 64 * 1024 * 1024;

/// The aspect-preserving downscale for a `width × height` image to fit
/// `max_w × max_h`: `(w, h, nw, nh)` where `(w, h)` are the source dims
/// guarded to `≥ 1` and `(nw, nh)` are the scaled dims. `None` when the
/// source already fits. Shared by the `image`-crate and the `simple`
/// box-filter resize paths.
#[must_use]
pub(crate) fn scale_fit(
    width: u32,
    height: u32,
    max_w: u32,
    max_h: u32,
) -> Option<(u32, u32, u32, u32)> {
    let w = width.max(1);
    let h = height.max(1);
    let scale =
        (max_w as f32 / w as f32).min(max_h as f32 / h as f32).min(1.0);
    if scale >= 1.0 {
        return None;
    }
    let nw = (w as f32 * scale).floor().max(1.0) as u32;
    let nh = (h as f32 * scale).floor().max(1.0) as u32;
    Some((w, h, nw, nh))
}

/// Decode an image to `(width, height, RGBA8)`.
#[cfg(feature = "simple")]
pub(crate) fn decode_image(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    simple::decode_png(bytes)
}

/// Decode an image to `(width, height, RGBA8)` via the `image` crate.
#[cfg(all(
    not(feature = "simple"),
    any(feature = "png", feature = "jpeg", feature = "gif", feature = "webp")
))]
pub(crate) fn decode_image(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let img = image::load_from_memory(bytes).ok()?;
    let (w, h) = (img.width(), img.height());
    let rgba = img.into_rgba8().into_raw();
    if rgba.len() > MAX_IMAGE_PAYLOAD {
        return None;
    }
    Some((w, h, rgba))
}

/// Expand raw RGB (3 bytes/px) into RGBA (4 bytes/px, `α = 255`).
#[cfg(feature = "simple")]
pub(crate) fn rgb_to_rgba(
    rgb: &[u8],
    width: u32,
    height: u32,
) -> Option<Vec<u8>> {
    let px = (width as usize).checked_mul(height as usize)?;
    let mut rgba = vec![0_u8; px * 4];
    simple::rgb_to_rgba(rgb, &mut rgba);
    Some(rgba)
}

/// Expand raw RGB (3 bytes/px) into RGBA (4 bytes/px, `α = 255`).
#[cfg(all(
    not(feature = "simple"),
    any(feature = "png", feature = "jpeg", feature = "gif", feature = "webp")
))]
pub(crate) fn rgb_to_rgba(
    rgb: &[u8],
    width: u32,
    height: u32,
) -> Option<Vec<u8>> {
    let img =
        image::RgbImage::from_raw(width.max(1), height.max(1), rgb.to_vec())?;
    Some(image::DynamicImage::ImageRgb8(img).into_rgba8().into_raw())
}

pub use parser::ImageParser;

pub use parser::{
    Image, ImageCtx, ImageKind, ImageParsers, ParseOutcome, ParsedImage,
    VirtualPlacement,
};

#[cfg(feature = "iterm2")]
pub use iterm2::{Iterm2Image, Resize, parse_iterm2, place_iterm2};
#[cfg(feature = "kitty")]
pub use kitty::{
    KittyImage, KittyMeta, KittyParser, decode_kitty_file, decode_kitty_raw,
    decode_kitty_rgba, parse_kitty_meta,
};
#[cfg(feature = "sixel")]
pub use sixel::{SixelImage, SixelParser, decode_sixel};

#[cfg(all(
    not(feature = "simple"),
    any(feature = "png", feature = "jpeg", feature = "gif", feature = "webp")
))]
pub use resize::scale_down_rgba;
#[cfg(feature = "simple")]
pub use simple::scale_down_rgba;

#[cfg(all(
    not(feature = "simple"),
    not(any(
        feature = "png",
        feature = "jpeg",
        feature = "gif",
        feature = "webp"
    ))
))]
compile_error!(
    "jetfish_image requires the `simple` feature or a format feature (png/jpeg/gif/webp)"
);
