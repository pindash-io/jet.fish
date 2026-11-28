// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Lifecycle facet: PTY polling, exit, OSC events, theme, clipboard.

use compact_str::CompactString;
use glam::USizeVec2;

use jetfish_protocol::{Activity, Error};

/// PTY polling, exit, OSC events, theme, and clipboard.
pub trait Lifecycle {
    /// Report the detected color scheme.
    fn set_color_scheme(&mut self, _scheme: u8) {}

    /// Apply a theme (the baked palette + defaults + selection highlight).
    fn set_theme(&mut self, _theme: &jetfish_protocol::Theme) {}

    /// The current default background color (viewport clear) as `[r,g,b,a]`.
    fn default_bg(&self) -> [f32; 4] {
        [0.0, 0.0, 0.0, 1.0]
    }

    /// The current default foreground (text) color, as `[r, g, b, a]`.
    fn default_fg(&self) -> [f32; 4] {
        [1.0, 1.0, 1.0, 1.0]
    }

    /// Regenerate the 256-color palette.
    fn regenerate_palette(&mut self, _harmonious: bool) {}

    /// Poll for new data; returns true to redraw.
    fn poll(&mut self) -> Result<bool, Error> {
        Ok(false)
    }

    /// Whether the child exited.
    fn exited(&self) -> bool {
        false
    }

    /// The layout generation counter.
    fn layout_generation(&self) -> u64 {
        0
    }

    /// Register the wake callback.
    fn set_wake(&mut self, _wake: Option<jetfish_protocol::WakeCallback>) {}

    /// A host-requested resize, if any.
    fn take_requested_resize(&mut self) -> Option<USizeVec2> {
        None
    }

    /// A queued notification, consumed once.
    fn take_notification(&mut self) -> Option<CompactString> {
        None
    }

    /// A queued cursor icon name, consumed once.
    fn take_mouse_cursor(&mut self) -> Option<CompactString> {
        None
    }

    /// Whether a BEL arrived, and clear it.
    fn take_bell(&mut self) -> bool {
        false
    }

    /// The selection highlight color.
    fn highlight_bg(&self) -> Option<[f32; 4]> {
        None
    }

    /// A queued window title, consumed once.
    fn take_title(&mut self) -> Option<CompactString> {
        None
    }

    /// The current window title (OSC 0/1/2), without consuming it.
    fn title(&self) -> Option<CompactString> {
        None
    }

    /// The shell running or idle state.
    fn activity(&self) -> Activity {
        Activity::Unknown
    }

    /// The last OSC 133 exit code.
    fn shell_exit_code(&self) -> Option<i32> {
        None
    }

    /// The current shell command line.
    fn shell_cmdline(&self) -> Option<&str> {
        None
    }

    /// The reported working directory (OSC 7), resolved from its `file://`
    /// URI to a filesystem path. Not existence-validated — the spawn layer
    /// falls back to `$HOME` when it no longer names a live directory.
    fn cwd(&self) -> Option<std::path::PathBuf> {
        None
    }

    /// A queued clipboard payload, consumed once.
    fn take_clipboard(&mut self) -> Option<Vec<u8>> {
        None
    }
}
