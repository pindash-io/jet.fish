// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Window padding: the gap between the window edge and the text grid.

/// The four window paddings, in logical pixels.
#[derive(Clone, Copy, Debug, PartialEq, serde::Deserialize)]
#[serde(default)]
pub struct Padding {
    /// Top padding (logical px).
    pub top: f32,
    /// Right padding (logical px).
    pub right: f32,
    /// Bottom padding (logical px).
    pub bottom: f32,
    /// Left padding (logical px).
    pub left: f32,
}

impl Padding {
    /// Zero padding on every side.
    pub const ZERO: Self =
        Self { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 };

    /// Each side clamped to non-negative.
    #[must_use]
    pub const fn clamped(self) -> Self {
        Self {
            top: self.top.max(0.0),
            right: self.right.max(0.0),
            bottom: self.bottom.max(0.0),
            left: self.left.max(0.0),
        }
    }
}

impl Default for Padding {
    fn default() -> Self {
        Self { top: 10.0, right: 10.0, bottom: 10.0, left: 10.0 }
    }
}
