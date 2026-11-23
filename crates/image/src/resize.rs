// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Image resizing for the host's fixed-size texture uploads.

use std::sync::Arc;

use image::imageops::FilterType;

/// Downscale RGBA pixels to fit within `max_w × max_h`, preserving aspect
/// ratio.
///
/// Returns `None` when the source already fits (no resize needed); otherwise
/// the scaled pixels and their new dimensions.
#[must_use]
pub fn scale_down_rgba(
    pixels: &[u8],
    width: u32,
    height: u32,
    max_w: u32,
    max_h: u32,
) -> Option<(Arc<[u8]>, u32, u32)> {
    let (w, h, nw, nh) = crate::scale_fit(width, height, max_w, max_h)?;
    let src = image::RgbaImage::from_raw(w, h, pixels.to_vec())?;
    let resized = image::imageops::resize(&src, nw, nh, FilterType::Triangle);
    Some((Arc::from(resized.into_raw()), nw, nh))
}

#[cfg(test)]
#[path = "../tests/resize/mod.rs"]
mod tests;
