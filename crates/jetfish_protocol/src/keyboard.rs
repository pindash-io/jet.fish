// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Mouse report encoding and alt-screen mode flags.
//!
//! The key-encoding model (`KeyEncoding`, `KeyEventKind`, `KeyLocation`,
//! `KeyModes`, `KeySpec`) lives in `vtty_core` alongside `KittyFlags`; the
//! winit-typed encoders live in `jetfish_runtime`.

/// Encode a mouse event under `tracking` (DEC 1016 converts cells to pixels).
/// `None` when tracking is off.
pub fn encode_mouse_report(
    button: crate::MouseButton,
    tracking: crate::MouseTracking,
    cell: crate::USizeVec2,
    mut x: u16,
    mut y: u16,
    pressed: bool,
) -> Option<Vec<u8>> {
    if tracking.contains(crate::MouseTracking::PIXEL) {
        let cw = u16::try_from(cell.x).unwrap_or(u16::MAX);
        let ch = u16::try_from(cell.y).unwrap_or(u16::MAX);
        x = x.saturating_mul(cw);
        y = y.saturating_mul(ch);
    }
    button.encode(tracking, x, y, pressed).map(|s| s.to_vec())
}

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

/// Decode an alt-screen mode toggle (`CSI ?47/1047/1049 h/l`) into the flags
/// the screens pass to their alt-screen enter/leave methods. `None` when `mode`
/// is not an alt-screen flag.
pub fn alt_screen_action(
    mode: crate::TermFlags,
    on: bool,
) -> Option<AltScreenSwitch> {
    if mode.contains(crate::TermFlags::ALT_SCREEN) {
        // 47: persistent grid, never saves the cursor or clears.
        return Some(AltScreenSwitch::empty());
    }
    if mode.contains(crate::TermFlags::ALT_SCREEN_1047) {
        // 1047: clears on EXIT (save = false, clear = !on).
        return Some(if on {
            AltScreenSwitch::empty()
        } else {
            AltScreenSwitch::CLEAR
        });
    }
    if mode.contains(crate::TermFlags::ALT_SCREEN_1049) {
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
