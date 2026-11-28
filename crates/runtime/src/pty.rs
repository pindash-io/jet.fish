// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! PTY + shell spawn over `rustix`, with a `libc` shim for the child's
//! signal-mask reset in `pre_exec`.

use std::io;
use std::os::fd::OwnedFd;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use compact_str::CompactString;

use rustix::fs::{Mode, OFlags};
use rustix::fs::{fcntl_getfl, fcntl_setfl, open};
use rustix::io::{FdFlags, fcntl_setfd};
use rustix::process::setsid;
use rustix::pty::OpenptFlags;
use rustix::pty::{grantpt, openpt, ptsname, unlockpt};
use rustix::termios::{
    ControlModes, InputModes, LocalModes, OptionalActions, OutputModes,
    SpecialCodeIndex, Termios, Winsize,
};
use rustix::termios::{tcgetattr, tcsetattr, tcsetwinsize};

/// Everything a child spawn needs beyond the command itself.
#[derive(Clone, Copy, Debug)]
pub struct PtyConfig<'a> {
    /// Initial terminal size in rows/cols; pixel dims are 0 until the first
    /// resize (the engine sets them from the real cell size).
    pub winsize: Winsize,
    /// Working directory for the child shell.
    pub cwd: Option<&'a Path>,
    /// Command (program + args) to run instead of the shell.
    pub command: Option<&'a [CompactString]>,
}

/// Open a PTY pair (master + slave, both `O_NOCTTY`). ENXIO means the OS has
/// no pseudo-terminal — typically a sandbox without /dev/ptmx.
pub fn open_pty() -> io::Result<(OwnedFd, OwnedFd)> {
    let master = openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY)?;
    // `posix_openpt` has no `O_CLOEXEC` on macOS; set it via fcntl everywhere.
    fcntl_setfd(&master, FdFlags::CLOEXEC)?;
    grantpt(&master)?;
    unlockpt(&master)?;
    let slave_path = ptsname(&master, Vec::new())?;
    let slave = open(
        &slave_path,
        OFlags::RDWR | OFlags::NOCTTY | OFlags::CLOEXEC,
        Mode::empty(),
    )?;
    Ok((master, slave))
}

/// Sane cooked (canonical) defaults, like `stty sane`. Full-screen apps set
/// raw mode themselves. `ISTRIP` stays off (clears bit 7, corrupting UTF-8);
/// `ECHOK` is omitted (macOS default is `ECHOKE`).
fn apply_sane_termios(t: &mut Termios) {
    t.input_modes
        .insert(InputModes::BRKINT | InputModes::ICRNL | InputModes::IXON);
    apply_utf8_input(t);
    t.output_modes.insert(OutputModes::OPOST | OutputModes::ONLCR);
    t.local_modes.insert(
        LocalModes::ICANON
            | LocalModes::ECHO
            | LocalModes::ECHOE
            | LocalModes::ECHOCTL
            | LocalModes::ISIG
            | LocalModes::IEXTEN,
    );
    t.control_modes.insert(ControlModes::CREAD | ControlModes::HUPCL);
    t.special_codes[SpecialCodeIndex::VERASE] = 0x7f; // DEL
    t.special_codes[SpecialCodeIndex::VINTR] = 0x03; // Ctrl-C
    t.special_codes[SpecialCodeIndex::VQUIT] = 0x1c; // Ctrl-\
    t.special_codes[SpecialCodeIndex::VEOF] = 0x04; // Ctrl-D
    t.special_codes[SpecialCodeIndex::VKILL] = 0x15; // Ctrl-U
    t.special_codes[SpecialCodeIndex::VSUSP] = 0x1a; // Ctrl-Z
}

/// `IUTF8` (`0x4000`): canonical-mode erase stays UTF-8 aware (a backspace
/// erases one code point, not one byte). rustix exposes `InputModes::IUTF8` on
/// Linux/macOS but not FreeBSD (raw bit there); `None` where the bit is absent.
const fn utf8_input_bit() -> Option<InputModes> {
    Some(cfg_select! {
        any(target_os = "linux", target_os = "macos") => InputModes::IUTF8,
        target_os = "freebsd" => InputModes::from_bits_retain(0x0000_4000),
        _ => return None,
    })
}

fn apply_utf8_input(t: &mut Termios) {
    let Some(bit) = utf8_input_bit() else {
        return;
    };
    t.input_modes.insert(bit);
}

/// Spawn `cmd` over a PTY at `config.winsize`. The slave starts in sane
/// cooked (canonical) mode with the standard control chars restored.
pub fn spawn(
    config: &PtyConfig<'_>,
    mut cmd: Command,
) -> io::Result<(OwnedFd, u32)> {
    if let Some(dir) = resolve_working_directory(config.cwd) {
        cmd.current_dir(dir);
    }
    let (master, slave) = open_pty()?;

    // Sane cooked mode + winsize.
    let mut t = tcgetattr(&slave)?;
    apply_sane_termios(&mut t);
    tcsetattr(&slave, OptionalActions::Now, &t)?;
    tcsetwinsize(&master, config.winsize)?;

    // Spawn `cmd` with stdio wired to the slave; `pre_exec` makes the slave
    // the controlling terminal.
    let slave_file = std::fs::File::from(slave);
    cmd.stdin(Stdio::from(slave_file.try_clone()?))
        .stdout(Stdio::from(slave_file.try_clone()?))
        .stderr(Stdio::from(slave_file))
        // Strip an inherited NO_COLOR; the shell re-exports it if intended.
        .env_remove("NO_COLOR")
        // `TERM` stays the generic `xterm-256color` for the widest shell/TUI
        // compatibility.
        .env("TERM", "xterm-256color")
        .env("COLORTERM", "truecolor")
        // For terminal detectors (fastfetch's `terminal` module, etc.).
        .env("TERM_PROGRAM", "jetfish")
        .env("TERM_PROGRAM_VERSION", env!("CARGO_PKG_VERSION"));
    // Run the child in a UTF-8 locale: `pbcopy` with `LC_CTYPE=C` decodes
    // UTF-8 as MacRoman (emoji → mojibake). A Finder launch has no `LANG`, so
    // set `LC_ALL` only when no locale var already picks one.
    if let Some((key, value)) = utf8_locale_env(
        std::env::var("LC_ALL").ok(),
        std::env::var("LC_CTYPE").ok(),
        std::env::var("LANG").ok(),
    ) {
        cmd.env(key, value);
    }
    // SAFETY: `pre_exec` runs in the forked child before `exec`; `setsid` and
    // the ioctl are async-signal-safe. The master is `CLOEXEC`, so it isn't
    // inherited.
    unsafe {
        cmd.pre_exec(move || {
            let _ = setsid();
            // A blocked SIGWINCH is inherited across fork+exec, leaving nvim's
            // handler permanently pending (never redraws); start with a clean
            // mask.
            let mut empty: libc::sigset_t = std::mem::zeroed();
            libc::sigemptyset(&mut empty);
            libc::sigprocmask(libc::SIG_SETMASK, &empty, std::ptr::null_mut());
            libc::signal(libc::SIGWINCH, libc::SIG_DFL);
            // Make stdin (the slave) the controlling terminal: without it the
            // shell's job control (`tcsetpgrp` on fd 0) fails with ENOTTY.
            //
            // SAFETY: fd 0 is the slave wired to stdin.
            let _ = rustix::process::ioctl_tiocsctty(
                rustix::fd::BorrowedFd::borrow_raw(0),
            );
            Ok(())
        });
    }
    let child = cmd.spawn()?;
    let pid = child.id();
    drop(child); // Child::drop does not wait; the pid lives on in `pid`.

    // Non-blocking master for the event loop / reader thread.
    let flags = fcntl_getfl(&master)?;
    fcntl_setfl(&master, flags | OFlags::NONBLOCK)?;

    Ok((master, pid))
}

/// Spawn the user's shell over a PTY (see [`spawn`]).
pub fn spawn_shell(config: &PtyConfig<'_>) -> io::Result<(OwnedFd, u32)> {
    let command = match config.command {
        Some([program, args @ ..]) => {
            let mut cmd = Command::new(program.as_str());
            cmd.args(args.iter().map(|a| a.as_str()));
            cmd
        }
        _ => Command::new(shell_path()),
    };
    spawn(config, command)
}

/// The child's start directory: `cwd` if it's a live dir, else `$HOME` (a
/// Finder/Dock launch has cwd `/`). `None` means "leave unset" (inherit).
fn resolve_working_directory(cwd: Option<&Path>) -> Option<PathBuf> {
    cwd.filter(|d| d.is_dir())
        .map(Path::to_path_buf)
        .or_else(std::env::home_dir)
        .filter(|d| d.is_dir())
}

/// The shell to run: `$SHELL`, else `/bin/sh`.
fn shell_path() -> PathBuf {
    std::env::var_os("SHELL")
        .filter(|s| !s.is_empty())
        .unwrap_or("/bin/sh".into())
        .into()
}

/// The locale override to force a UTF-8 char encoding when neither `LC_ALL`,
/// `LC_CTYPE`, nor `LANG` already picks one. `None` keeps the inherited
/// environment.
fn utf8_locale_env(
    lc_all: Option<String>,
    lc_ctype: Option<String>,
    lang: Option<String>,
) -> Option<(&'static str, &'static str)> {
    let is_utf8 = |v: &Option<String>| {
        v.as_deref().is_some_and(|s| s.to_ascii_lowercase().contains("utf"))
    };
    if !is_utf8(&lc_all) && !is_utf8(&lc_ctype) && !is_utf8(&lang) {
        Some(("LC_ALL", "en_US.UTF-8"))
    } else {
        None
    }
}

#[cfg(test)]
#[path = "../tests/pty/mod.rs"]
mod tests;
