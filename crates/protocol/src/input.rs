// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Key/mouse report encoding and alt-screen mode flags.
//!
//! The key-encoding model (`KeyEncoding`, `KeyEventKind`, `KeyLocation`,
//! `KeyModes`, `KeySpec`) lives in `vtty_core` alongside `KittyFlags`; the
//! winit-typed encoders live in `jetfish_runtime`.

use glam::USizeVec2;

use vtty::{KeyEncoding, MouseButton, MouseTracking, TermFlags};

bitflags::bitflags! {
    /// The two flags an alt-screen enter or leave carries.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct AltScreenSwitch: u8 {
        /// Save or restore the cursor (the DECSC half of `?1049`).
        const SAVE = 1;
        /// Clear the grid on entry or exit.
        const CLEAR = 2;
    }
}

/// Select the press encoding from the active kitty flags and the DECCKM
/// (`?1h`) cursor-keys state: kitty wins when its flags are set, then DECCKM
/// application cursor keys (`SS3 A`), else the normal CSI legacy form.
pub const fn select_press_encoding(
    keyboard_flags: u8,
    application_cursor: bool,
) -> KeyEncoding {
    if keyboard_flags != 0 {
        return KeyEncoding::Kitty(keyboard_flags);
    }
    if application_cursor {
        return KeyEncoding::ApplicationCursorKeys;
    }
    KeyEncoding::Legacy
}

/// Encode a mouse event under `tracking` (DEC 1016 converts cells to pixels).
/// `None` when tracking is off.
#[must_use]
pub fn encode_mouse_report(
    button: MouseButton,
    tracking: MouseTracking,
    cell: USizeVec2,
    mut x: u16,
    mut y: u16,
    pressed: bool,
) -> Option<Vec<u8>> {
    if tracking.contains(MouseTracking::PIXEL) {
        let cw = u16::try_from(cell.x).unwrap_or(u16::MAX);
        let ch = u16::try_from(cell.y).unwrap_or(u16::MAX);
        x = x.saturating_mul(cw);
        y = y.saturating_mul(ch);
    }
    button.encode(tracking, x, y, pressed).map(|s| s.to_vec())
}

/// Decode an alt-screen mode toggle (`CSI ?47/1047/1049 h/l`) into the flags
/// the screens pass to their alt-screen enter/leave methods. `None` when `mode`
/// is not an alt-screen flag.
#[must_use]
pub fn alt_screen_action(
    mode: TermFlags,
    on: bool,
) -> Option<AltScreenSwitch> {
    if mode.contains(TermFlags::ALT_SCREEN) {
        // 47: persistent grid, never saves the cursor or clears.
        return Some(AltScreenSwitch::empty());
    }
    if mode.contains(TermFlags::ALT_SCREEN_1047) {
        // 1047: clears on EXIT (save = false, clear = !on).
        return Some(if on {
            AltScreenSwitch::empty()
        } else {
            AltScreenSwitch::CLEAR
        });
    }
    if mode.contains(TermFlags::ALT_SCREEN_1049) {
        // 1049: saves/restores the cursor, clears on ENTRY (save = true,
        // clear = on).
        return Some(if on {
            AltScreenSwitch::SAVE | AltScreenSwitch::CLEAR
        } else {
            AltScreenSwitch::SAVE
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use vtty::{KeyEncoding, KittyFlags};

    use super::select_press_encoding;

    /// DECCKM (`?1h`) must select the application-cursor form for a legacy
    /// press; kitty flags always win, and the DECCKM-off form is CSI.
    #[test]
    fn press_encoding_respects_decckm() {
        assert_eq!(select_press_encoding(0, false), KeyEncoding::Legacy);
        assert_eq!(
            select_press_encoding(0, true),
            KeyEncoding::ApplicationCursorKeys
        );
        assert_eq!(
            select_press_encoding(KittyFlags::DISAMBIGUATE.bits(), true),
            KeyEncoding::Kitty(KittyFlags::DISAMBIGUATE.bits())
        );
    }
}
