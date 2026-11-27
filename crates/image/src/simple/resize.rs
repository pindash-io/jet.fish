// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Image resizing for the host's fixed-size texture uploads.

use std::sync::Arc;

use fearless_simd::{Level, Simd};
use fearless_simd::{dispatch, prelude::*, u32x4};

/// Box-filter downscale of RGBA pixels to fit `max_w × max_h`, preserving
/// aspect; alpha averages premultiplied. `None` when it already fits.
#[must_use]
pub fn scale_down_rgba(
    pixels: &[u8],
    width: u32,
    height: u32,
    max_w: u32,
    max_h: u32,
) -> Option<(Arc<[u8]>, u32, u32)> {
    let (w, h, nw, nh) = crate::scale_fit(width, height, max_w, max_h)?;
    let mut out = vec![0_u8; (nw * nh * 4) as usize];
    let level = Level::new();
    dispatch!(level, simd => scale_simd(simd, pixels, w, h, nw, nh, &mut out));
    Some((Arc::from(out), nw, nh))
}

/// One box-summed output pixel: average the premultiplied RGBA over the
/// source box `[sx0..sx1) × [sy0..sy1)` (four pixels per SIMD lane).
#[inline(always)]
#[allow(
    clippy::too_many_arguments,
    reason = "the SIMD box bounds form one cohesive call"
)]
fn scale_simd<S: Simd>(
    simd: S,
    pixels: &[u8],
    width: u32,
    height: u32,
    nw: u32,
    nh: u32,
    out: &mut [u8],
) {
    let mask = u32x4::splat(simd, 0xFF);
    for dy in 0..nh {
        let sy0 = dy * height / nh;
        let sy1 = (dy + 1) * height / nh;
        for dx in 0..nw {
            let sx0 = dx * width / nw;
            let sx1 = (dx + 1) * width / nw;
            // Premultiplied channel sums. The four u32 lanes accumulate
            // four independent pixels; the scalar tail covers the rest.
            let mut r = u32x4::splat(simd, 0);
            let mut g = u32x4::splat(simd, 0);
            let mut b = u32x4::splat(simd, 0);
            let mut a = u32x4::splat(simd, 0);
            let mut tail = [0_u64; 4];
            for sy in sy0..sy1 {
                let row = &pixels[(sy * width) as usize * 4..];
                let mut sx = sx0;
                while sx + 4 <= sx1 {
                    let base = sx as usize * 4;
                    let px = u32x4::simd_from(
                        simd,
                        [
                            u32::from_le_bytes([
                                row[base],
                                row[base + 1],
                                row[base + 2],
                                row[base + 3],
                            ]),
                            u32::from_le_bytes([
                                row[base + 4],
                                row[base + 5],
                                row[base + 6],
                                row[base + 7],
                            ]),
                            u32::from_le_bytes([
                                row[base + 8],
                                row[base + 9],
                                row[base + 10],
                                row[base + 11],
                            ]),
                            u32::from_le_bytes([
                                row[base + 12],
                                row[base + 13],
                                row[base + 14],
                                row[base + 15],
                            ]),
                        ],
                    );
                    let alpha = (px >> 24) & mask;
                    r += (px & mask) * alpha;
                    g += ((px >> 8) & mask) * alpha;
                    b += ((px >> 16) & mask) * alpha;
                    a += alpha;
                    sx += 4;
                }
                for sx in sx..sx1 {
                    let idx = (sy as usize * width as usize + sx as usize) * 4;
                    let alpha = u64::from(pixels[idx + 3]);
                    tail[0] += u64::from(pixels[idx]) * alpha;
                    tail[1] += u64::from(pixels[idx + 1]) * alpha;
                    tail[2] += u64::from(pixels[idx + 2]) * alpha;
                    tail[3] += alpha;
                }
            }
            // Reduce the four lanes into the scalar tail.
            let sum_r = u64::from(r[0] + r[1] + r[2] + r[3]) + tail[0];
            let sum_g = u64::from(g[0] + g[1] + g[2] + g[3]) + tail[1];
            let sum_b = u64::from(b[0] + b[1] + b[2] + b[3]) + tail[2];
            let sum_a = u64::from(a[0] + a[1] + a[2] + a[3]) + tail[3];
            let area = u64::from((sy1 - sy0) * (sx1 - sx0));
            let oidx = ((dy * nw + dx) * 4) as usize;
            out[oidx] = sum_r.checked_div(sum_a).unwrap_or(0) as u8;
            out[oidx + 1] = sum_g.checked_div(sum_a).unwrap_or(0) as u8;
            out[oidx + 2] = sum_b.checked_div(sum_a).unwrap_or(0) as u8;
            out[oidx + 3] = (sum_a / area) as u8;
        }
    }
}

#[cfg(test)]
#[path = "../../tests/simple/resize.rs"]
mod tests;
