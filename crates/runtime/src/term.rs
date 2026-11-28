// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Generic terminal engine shell: `Term` holds the PTY, parser, and input
//! path. Screen-specific render state lives in the `E` extension slot.

use std::os::fd::{AsFd as _, OwnedFd};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, mpsc::Receiver};
use std::time::{Duration, Instant};

use compact_str::CompactString;
use glam::{U16Vec2, UVec2, Vec2};
use snafu::ResultExt as _;

use vtty::Parser;
use vtty::encode_key;
use vtty::{
    Key, KeyEncoding, KeyEventKind, KeyModes, KittyFlags, Modifiers,
    MouseButton,
};

use jetfish_protocol::{Result, WakeSlot};
use jetfish_protocol::{cells_to_px, select_press_encoding, write_fd_all};
use jetfish_screen::TermInput;

/// Engine extension slot: the screen-specific per-engine state the generic
/// [`Term`] shell carries without knowing its shape.
pub trait TermExt {}

/// The default engine extension (no per-screen state).
impl TermExt for () {}

/// Layout debounce: whether a layout is pending and how quickly it settles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cadence {
    /// No pending layout.
    Idle,
    /// Interactive input (key echo/repeat, prompt, paste) — repaint on the
    /// next frame (~16ms).
    Interactive,
    /// Continuous bulk output (`cat`/`scp`) — coalesce to ~150ms.
    Streaming,
}

impl Cadence {
    /// Whether a layout is pending.
    pub const fn is_pending(&self) -> bool {
        !matches!(self, Self::Idle)
    }
}

/// A terminal engine: one PTY + parser + input path, plus the screen-specific
/// `E` extension.
pub struct Term<SCREEN: TermInput, E: TermExt = ()> {
    /// Unique engine id (for log correlation across surfaces/windows).
    pub id: u64,
    /// The screen this engine drives (`Screen<CellState, CellExtend>`).
    pub state: SCREEN,
    /// The VT parser (SIMD fast paths).
    pub parser: Parser,
    /// Decoded PTY output channel (termio → engine).
    pub pty_rx: Receiver<Vec<u8>>,
    /// The PTY master fd.
    pub pty: OwnedFd,
    /// PTY child pid — SIGHUP'd on drop; kept (not waited) so the kill can't
    /// block the render loop.
    pub child: u32,
    /// The layout debounce: idle / interactive / streaming.
    pub debounce: Cadence,
    /// Debounce: last time PTY data arrived (persists across idle/pending).
    pub last_data: Instant,
    /// Wake callback for the event loop (PTY reader invokes it).
    pub wake: WakeSlot,
    /// Set when the PTY reader sees EOF (the shell exited).
    pub exited: Arc<AtomicBool>,
    /// Screen-specific engine state.
    pub extend: E,
}

impl<SCREEN: TermInput, E: TermExt> Term<SCREEN, E> {
    /// Spawn a shell PTY + reader and assemble the engine shell around the
    /// given screen and extension.
    pub fn spawn(
        config: &crate::pty::PtyConfig<'_>,
        state: SCREEN,
        extend: E,
    ) -> Result<Self> {
        let (pty, child) = crate::pty::spawn_shell(config)
            .whatever_context::<_, jetfish_protocol::Error>(
                "open the pseudo-terminal (is /dev/ptmx available? a sandboxed process has no PTY)",
            )?;
        let wake: WakeSlot = Arc::new(Mutex::new(None));
        let id = crate::utils::next_engine_id();
        let (rx, exited) = crate::register(&pty, wake.clone(), Some(id))?;
        Ok(Self {
            id,
            state,
            parser: Parser::with_simd(),
            pty_rx: rx,
            pty,
            child,
            debounce: Cadence::Idle,
            last_data: Instant::now() - Duration::from_secs(1),
            wake,
            exited,
            extend,
        })
    }

    /// Encode a key press with the active protocol (kitty, else legacy/DECCKM).
    #[must_use]
    pub fn encode_input_key(
        &self,
        key: &Key,
        mods: Modifiers,
    ) -> Option<CompactString> {
        let encoding = select_press_encoding(
            self.state.keyboard_flags(),
            self.state.application_cursor(),
        );
        encode_key(
            key,
            mods,
            encoding,
            KeyEventKind::Press,
            self.key_modes(),
            None,
        )
    }

    /// Write the encoded key bytes to the PTY (with the `pty.in` log line).
    pub fn write_key(&self, key: &Key, mods: Modifiers, b: &[u8]) {
        // No dedup: legitimate repeats (`gg`, `dd`, `yy` in vim) arrive under
        // 5ms apart and must not be swallowed; winit's `is_synthetic` handles
        // platform duplication.
        jetfish_log::log!(
            "pty.in",
            "eng={} key={:?} mods={:?} bytes: {}",
            self.id,
            key,
            mods,
            jetfish_log::log::escape_bytes(b)
        );
        self.write_pty(b);
    }

    /// Write raw bytes to the PTY.
    pub fn write_pty(&self, bytes: &[u8]) {
        write_fd_all(self.pty.as_fd(), bytes);
    }

    /// Encode a press-only key and write it to the PTY (with the `pty.in`
    /// log line).
    pub fn write_encoded_key(&self, key: &Key, mods: Modifiers) {
        let Some(b) = self.encode_input_key(key, mods) else {
            return;
        };
        self.write_key(key, mods, b.as_bytes());
    }

    /// The grid size in cells (`.x` = cols, `.y` = rows), read from the screen.
    pub fn grid(&self) -> U16Vec2 {
        self.state.grid().as_u16vec2()
    }

    /// The grid cell size in whole pixels, read from the screen.
    pub fn cell_size(&self) -> Vec2 {
        self.state.cell_px().as_vec2()
    }

    /// The exact window size for the current grid, rounded to the NEAREST
    /// pixel (not ceil) so the grid fills the viewport edge-to-edge.
    #[must_use]
    pub fn preferred_size(&self) -> UVec2 {
        cells_to_px(self.grid().as_uvec2(), self.cell_size())
            .round()
            .as_uvec2()
    }

    /// Encode a full key event; kitty emits `report_events`/`report_text`,
    /// else the press-only legacy path.
    #[must_use]
    pub fn encode_input_key_event(
        &self,
        key: &Key,
        mods: Modifiers,
        kind: KeyEventKind,
        text: Option<&str>,
    ) -> Option<CompactString> {
        let flags = self.state.keyboard_flags();
        let encoding = if flags == 0 {
            if kind != KeyEventKind::Press {
                return None;
            }
            select_press_encoding(flags, self.state.application_cursor())
        } else {
            KeyEncoding::Kitty(flags)
        };
        encode_key(key, mods, encoding, kind, self.key_modes(), text)
    }

    /// The keypad/backarrow DEC modes for the encoder.
    fn key_modes(&self) -> KeyModes {
        KeyModes {
            application_keypad: self.state.application_keypad(),
            numlock_ignored: self.state.numlock_ignored(),
            backarrow: self.state.backarrow(),
        }
    }

    /// kitty `report_all_keys` (flag 8): raw modifier-key events are wanted.
    pub fn kitty_report_all_keys(&self) -> bool {
        self.state.keyboard_flags() & KittyFlags::REPORT_ALL_KEYS.bits() != 0
    }

    /// Encode a mouse report (SGR/X10) using the active tracking modes and the
    /// whole-pixel cell size. `x`/`y` are 0-based cell coordinates; DEC 1016
    /// converts them to pixels.
    #[must_use]
    pub fn encode_input_mouse(
        &self,
        button: MouseButton,
        x: u16,
        y: u16,
        pressed: bool,
    ) -> Option<Vec<u8>> {
        jetfish_protocol::encode_mouse_report(
            button,
            self.state.mouse_tracking(),
            self.state.cell_px(),
            x,
            y,
            pressed,
        )
    }
}

impl<SCREEN: TermInput, E: TermExt> Drop for Term<SCREEN, E> {
    fn drop(&mut self) {
        terminate_shell(&self.pty, self.child);
    }
}

/// HUP the shell's process groups and reap it (SIGHUP is catchable, so loop
/// up to ~100ms).
fn terminate_shell(pty: &OwnedFd, child: u32) {
    use rustix::process::{Pid, Signal, WaitOptions};
    use rustix::process::{getpgid, kill_process_group, waitpid};

    let Some(shell) = Pid::from_raw(child as i32) else {
        return;
    };

    fn hup_pgid(pgid: i32) {
        if pgid <= 0 {
            return;
        }
        let Some(pgid) = Pid::from_raw(pgid) else {
            return;
        };
        let _ = kill_process_group(pgid, Signal::HUP);
    }

    // Signal the PTY's foreground job (e.g. herdr), then the shell's own
    // process group (background jobs).
    hup_pgid(foreground_pgrp(pty));
    if let Ok(pgid) = getpgid(Some(shell)) {
        let _ = kill_process_group(pgid, Signal::HUP);
    }

    // Reap the shell so it doesn't linger as a zombie. SIGHUP is catchable,
    // so loop (killpg + non-blocking wait) until reaped, capped at 100ms.
    let deadline = Instant::now() + Duration::from_millis(100);
    loop {
        match waitpid(Some(shell), WaitOptions::NOHANG) {
            Ok(Some(_)) | Err(_) => break, // reaped, or already gone
            Ok(None) => {
                if Instant::now() >= deadline {
                    break;
                }
                if let Ok(pgid) = getpgid(Some(shell)) {
                    let _ = kill_process_group(pgid, Signal::HUP);
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
}

/// The PTY slave's foreground process group via TIOCGPGRP, or `0` on failure.
/// Reads the raw value (rustix's `tcgetpgrp` is UB when the pgrp is 0);
/// `_IOR('t', 119)` is the same on Linux and BSD/macOS, so no cfg is needed.
fn foreground_pgrp(fd: &OwnedFd) -> i32 {
    use rustix::fd::AsFd;
    use rustix::ioctl::{Getter, ioctl, opcode};

    const TIOCGPGRP: rustix::ioctl::Opcode = opcode::read::<i32>(b't', 119);
    // SAFETY: `fd` is a valid open PTY master; TIOCGPGRP writes an `i32` that
    // the getter owns.
    let getter = unsafe { Getter::<TIOCGPGRP, i32>::new() };
    unsafe { ioctl(fd.as_fd(), getter) }.unwrap_or_default()
}
#[cfg(test)]
#[path = "../tests/term/mod.rs"]
mod tests;
