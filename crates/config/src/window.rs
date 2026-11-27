// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Window settings: the [`Window`] config section.

use serde::Deserialize;

use jetfish_protocol::Padding;

/// Window settings.
#[derive(Deserialize, Debug)]
#[serde(default)]
pub struct Window {
    /// Opacity 0.0..=1.0.
    #[serde(default = "default_opacity")]
    pub opacity: f32,
    /// The gap between the window edge and the text grid (logical px).
    pub padding: Padding,
}

impl Default for Window {
    fn default() -> Self {
        Self { opacity: 1.0, padding: Padding::default() }
    }
}

const fn default_opacity() -> f32 {
    1.0
}
