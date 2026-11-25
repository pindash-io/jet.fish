// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Input facet: keyboard, mouse, paste, focus, and tracking-mode queries.

/// Keyboard, mouse, paste, focus, and tracking-mode queries.
pub trait Input {
    /// Keyboard modifiers state.
    type Keyboard: Default;
    /// A key event.
    type Event;

    /// Handle a key press.
    fn handle_key(&mut self, _event: &Self::Event, _keyboard: Self::Keyboard) {
    }

    /// Handle a full key event with the typed text; the default forwards to
    /// [`Self::handle_key`].
    fn handle_key_event(
        &mut self,
        event: &Self::Event,
        keyboard: Self::Keyboard,
        text: Option<&str>,
    ) {
        let _ = text;
        self.handle_key(event, keyboard);
    }

    /// Whether raw modifier-key events are wanted.
    fn wants_modifier_keys(&self) -> bool {
        false
    }

    /// Paste text into the terminal.
    fn handle_paste(&mut self, _text: &str) {}

    /// Handle a mouse event at a cell.
    fn handle_mouse(
        &mut self,
        _button: vtty::MouseButton,
        _x: u16,
        _y: u16,
        _pressed: bool,
    ) {
    }

    /// Report a focus change.
    fn report_focus(&mut self, _focused: bool) {}

    /// Set whether this surface's cursor renders as active (solid + blinking).
    /// The window layer folds the OS window focus AND the split-pane focus
    /// into this one flag, so only the single active pane is active at a time.
    /// Distinct from `report_focus` (the OS window focus + DEC 1004 report):
    /// activation is visual-only and never emits a DEC 1004 report.
    fn set_activated(&mut self, _activated: bool) {}

    /// Show the cursor; resets the blink timer.
    fn reset_cursor_blink(&mut self) {}

    /// The active mouse tracking mode.
    fn mouse_mode(&self) -> u16 {
        0
    }

    /// Whether mouse tracking is on.
    fn mouse_tracking(&self) -> bool {
        self.mouse_mode() != 0
    }

    /// Whether DEC 1003 any-motion tracking is on.
    fn mouse_any_motion(&self) -> bool {
        self.mouse_mode() == 1003
    }

    /// Whether DEC 1002 button-motion tracking is on.
    fn mouse_button_motion(&self) -> bool {
        self.mouse_mode() == 1002
    }

    /// Whether DEC 2026 synchronized output is active.
    fn synchronized_output_active(&self) -> bool {
        false
    }

    /// Whether output is framed: explicit DEC 2026 sync, or the alt
    /// screen's implicit post-resize frame (the "old format" TUI redraw).
    fn framed_output_active(&self) -> bool {
        self.synchronized_output_active()
    }
}

/// A key event: the key, press state, and location.
#[derive(Clone, Debug)]
pub struct KeyEvent {
    /// The key.
    pub key: vtty::Key,
    /// Press / repeat / release.
    pub kind: vtty::KeyEventKind,
    /// Standard / numpad / function-row location.
    pub location: vtty::KeyLocation,
}
