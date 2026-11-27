// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Typed input-mode state: kitty keyboard flags and
//! mouse tracking. Plain owned fields of the screen.

use vtty::{KittyFlags, MouseTracking, TermFlags};

/// Kitty keyboard protocol state: active flags plus the push/pop stack.
#[derive(Debug, Default)]
pub struct KittyKeyboard {
    /// The flags currently in effect (top of stack).
    pub flags: KittyFlags,
    /// The flag stack for `CSI >u` / `CSI <u`.
    pub stack: Vec<KittyFlags>,
}

impl KittyKeyboard {
    /// `CSI > flags u`: push the current flags and activate `flags`.
    #[inline]
    pub fn push(&mut self, flags: u8) {
        self.stack.push(self.flags);
        self.flags = KittyFlags::from_bits_truncate(flags);
    }

    /// `CSI = flags ; mode u`: set, insert or remove (`mode` 1/2/3).
    #[inline]
    pub fn set(&mut self, flags: u8, mode: u8) {
        let f = KittyFlags::from_bits_truncate(flags);
        match mode {
            2 => self.flags.insert(f),
            3 => self.flags.remove(f),
            _ => self.flags = f,
        }
    }

    /// `CSI < count u`: pop one level (an empty stack restores no flags).
    #[inline]
    pub fn pop(&mut self) {
        self.flags = self.stack.pop().unwrap_or(KittyFlags::empty());
    }

    /// DECSTR / RIS: drop the active flags and the whole stack.
    #[inline]
    pub fn reset(&mut self) {
        self.flags = KittyFlags::empty();
        self.stack.clear();
    }
}

/// Mouse tracking state. The tracking mode is a single `u16`; the protocol
/// flags reuse [`MouseTracking`] bitflags.
#[derive(Clone, Copy, Debug)]
pub struct Mouse {
    /// Tracking mode: 0 = off, 9 = X10, 1000 = press, 1002 = drag,
    /// 1003 = any motion. The modes are mutually exclusive.
    pub mode: u16,
    /// Protocol flags (DEC 1006 SGR, 1005 UTF-8, 1016 pixel): the mode-free half
    /// of the vtty [`MouseTracking`] summary.
    pub flags: MouseTracking,
    /// Focus in/out reporting (DEC 1004).
    pub focus: bool,
    /// Alternate scroll (DEC 1007): wheel sends arrow keys.
    pub alternate_scroll: bool,
}

impl Mouse {
    /// The default mouse state: tracking off, no protocol flags, no focus
    /// reporting, no alternate scroll.
    pub const DEFAULT: Self = Self {
        mode: 0,
        flags: MouseTracking::empty(),
        focus: false,
        alternate_scroll: false,
    };

    /// Tracking modes by DEC private-mode number (9 / 1000 / 1002 / 1003), which
    /// are mutually exclusive; `0` is off.
    pub const MODE_OFF: u16 = 0;
    /// X10 (DEC 9): button press only.
    pub const MODE_X10: u16 = 9;
    /// Normal tracking (DEC 1000): press + release.
    pub const MODE_NORMAL: u16 = 1000;
    /// Button-event tracking (DEC 1002): press + release + button-held motion.
    pub const MODE_BUTTON: u16 = 1002;
    /// Any-event tracking (DEC 1003): every motion event.
    pub const MODE_ANY: u16 = 1003;

    /// The full tracking summary: the protocol flags plus the bits derived
    /// from the exclusive tracking [`Self::mode`].
    #[inline]
    #[must_use]
    pub fn tracking(&self) -> MouseTracking {
        let mut t = self.flags;
        t.set(MouseTracking::ENABLED, self.mode != Self::MODE_OFF);
        t.set(MouseTracking::X10, self.mode == Self::MODE_X10);
        t.set(MouseTracking::ANY_MOTION, self.mode == Self::MODE_ANY);
        t
    }

    /// SGR mouse protocol (DEC 1006).
    #[inline]
    pub const fn sgr(&self) -> bool {
        self.flags.contains(MouseTracking::SGR)
    }

    /// Set the SGR mouse protocol flag (DEC 1006).
    #[inline]
    pub fn set_sgr(&mut self, on: bool) {
        self.flags.set(MouseTracking::SGR, on);
    }

    /// UTF-8 mouse reporting (DEC 1005).
    #[inline]
    pub const fn utf8(&self) -> bool {
        self.flags.contains(MouseTracking::UTF8)
    }

    /// Set the UTF-8 mouse flag (DEC 1005).
    #[inline]
    pub fn set_utf8(&mut self, on: bool) {
        self.flags.set(MouseTracking::UTF8, on);
    }

    /// SGR-pixel mouse protocol (DEC 1016).
    #[inline]
    pub const fn pixel(&self) -> bool {
        self.flags.contains(MouseTracking::PIXEL)
    }

    /// Set the SGR-pixel mouse flag (DEC 1016).
    #[inline]
    pub fn set_pixel(&mut self, on: bool) {
        self.flags.set(MouseTracking::PIXEL, on);
    }

    /// The DEC 1004 focus report (`CSI I` focus-in / `CSI O` focus-out),
    /// or `None` when focus reporting is disabled.
    #[inline]
    pub const fn focus_report(&self, focused: bool) -> Option<&'static [u8]> {
        if !self.focus {
            return None;
        }
        Some(if focused { b"\x1b[I" } else { b"\x1b[O" })
    }

    /// DECSTR (soft reset): clear the focus / UTF-8 / alternate-scroll modes.
    /// The exclusive tracking mode and the SGR/pixel flags survive DECSTR.
    #[inline]
    pub fn soft_reset(&mut self) {
        self.focus = false;
        self.set_utf8(false);
        self.alternate_scroll = false;
    }

    /// The state of a mouse-tracking mode for DECRQM (`CSI ?Ps$p`), or `None`
    /// when `mode` is not a mouse mode.
    #[must_use]
    #[inline]
    pub const fn mode_state(&self, mode: TermFlags) -> Option<bool> {
        if mode.contains(TermFlags::MOUSE_X10) {
            return Some(self.mode == Self::MODE_X10);
        }
        if mode.contains(TermFlags::MOUSE_NORMAL) {
            return Some(self.mode == Self::MODE_NORMAL);
        }
        if mode.contains(TermFlags::MOUSE_BUTTON) {
            return Some(self.mode == Self::MODE_BUTTON);
        }
        if mode.contains(TermFlags::MOUSE_ANY) {
            return Some(self.mode == Self::MODE_ANY);
        }
        if mode.contains(TermFlags::MOUSE_SGR) {
            return Some(self.sgr());
        }
        if mode.contains(TermFlags::FOCUS) {
            return Some(self.focus);
        }
        if mode.contains(TermFlags::UTF8_MOUSE) {
            return Some(self.utf8());
        }
        if mode.contains(TermFlags::ALTERNATE_SCROLL) {
            return Some(self.alternate_scroll);
        }
        if mode.contains(TermFlags::MOUSE_PIXEL) {
            return Some(self.pixel());
        }
        None
    }

    /// Apply a mouse-tracking toggle (`CSI ?Ps h/l`): an exclusive mode, a
    /// protocol flag, focus reporting or alternate scroll. `true` if consumed.
    #[must_use]
    #[inline]
    pub fn set_mode(&mut self, mode: TermFlags, on: bool) -> bool {
        if mode.contains(TermFlags::MOUSE_X10) {
            self.mode = if on { Self::MODE_X10 } else { Self::MODE_OFF };
            return true;
        }
        if mode.intersects(
            TermFlags::MOUSE_NORMAL
                | TermFlags::MOUSE_BUTTON
                | TermFlags::MOUSE_ANY,
        ) {
            self.mode = if on {
                u16::try_from(mode.private_param()).unwrap_or(Self::MODE_OFF)
            } else {
                Self::MODE_OFF
            };
            return true;
        }
        if mode.contains(TermFlags::MOUSE_SGR) {
            self.set_sgr(on);
            return true;
        }
        if mode.contains(TermFlags::FOCUS) {
            self.focus = on;
            return true;
        }
        if mode.contains(TermFlags::UTF8_MOUSE) {
            self.set_utf8(on);
            return true;
        }
        if mode.contains(TermFlags::ALTERNATE_SCROLL) {
            self.alternate_scroll = on;
            return true;
        }
        if mode.contains(TermFlags::MOUSE_PIXEL) {
            self.set_pixel(on);
            return true;
        }
        false
    }
}
