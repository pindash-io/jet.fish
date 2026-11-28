// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The runtime-pluggable screen backend.

use std::path::Path;

use compact_str::CompactString;

use vtty::{Key, Modifiers};

use jetfish_protocol::Result;

use crate::engine::TextEngine;

/// The concrete terminal engine: driven with protocol key events.
pub type Engine<K = Modifiers, E = Key> = Box<dyn TextEngine<K, E> + 'static>;

/// Everything a backend needs to spawn a shell for a new surface.
#[derive(Clone, Debug)]
pub struct SpawnConfig<'a> {
    /// Working directory for the child shell.
    pub cwd: Option<&'a Path>,
    /// Command (program + args) to run instead of the shell.
    pub command: Option<&'a [CompactString]>,
}

/// The runtime-pluggable screen backend factory: builds an engine from the
/// shared spawn config.
pub trait ScreenBackend: Sync {
    /// Spawn an engine for the surface.
    fn spawn(&self, config: &SpawnConfig<'_>) -> Result<Engine>;
}
