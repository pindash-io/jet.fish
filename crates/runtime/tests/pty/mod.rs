// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

/// The cooked-output newline contract: `OPOST | ONLCR` maps the shell's bare
/// `\n` to `\r\n` (column 0); raw-mode TUIs clear ONLCR themselves.
#[test]
fn sane_termios_enables_cooked_output() {
    // Skip (not fail) when the OS has no PTY — sandboxed CI/agents
    // without /dev/ptmx return ENXIO from `openpt`.
    let Ok(master) = openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY) else {
        eprintln!("no pseudo-terminal available; skipping");
        return;
    };
    grantpt(&master).unwrap();
    unlockpt(&master).unwrap();
    let slave_path = ptsname(&master, Vec::new()).unwrap();
    let slave =
        open(&slave_path, OFlags::RDWR | OFlags::NOCTTY, Mode::empty())
            .unwrap();

    let mut t = tcgetattr(&slave).unwrap();
    apply_sane_termios(&mut t);

    assert!(
        !t.input_modes.contains(InputModes::ISTRIP),
        "ISTRIP must stay off: it clears bit 7 of input bytes, corrupting \
         multi-byte UTF-8 (emoji paste)"
    );
    assert_utf8_input(&t);
    assert!(
        t.output_modes.contains(OutputModes::OPOST),
        "cooked output processing must be on"
    );
    assert!(
        t.output_modes.contains(OutputModes::ONLCR),
        "bare LF must map to CRLF (column 0) for cooked-mode programs"
    );
    assert!(
        t.local_modes.contains(LocalModes::ISIG),
        "signal chars (Ctrl-C) must stay enabled"
    );
    assert!(
        t.local_modes.contains(LocalModes::ECHO),
        "the kernel must echo typed bytes (Linux bash relies on it)"
    );
    assert!(
        t.control_modes.contains(ControlModes::CREAD | ControlModes::HUPCL),
        "receiver enabled and hangup-on-close set"
    );
}

/// `IUTF8` keeps canonical-mode erase UTF-8 aware; `utf8_input_bit` returns
/// the platform's bit (or `None` where it isn't available).
fn assert_utf8_input(t: &Termios) {
    let Some(bit) = utf8_input_bit() else { return };
    assert!(
        t.input_modes.contains(bit),
        "IUTF8 must be on: canonical-mode backspace erases a whole UTF-8 \
             code point instead of one byte"
    );
}

/// Drain the non-blocking master until EOF/timeout, returning its output.
fn drain_master(mut master: std::fs::File) -> Vec<u8> {
    use std::io::Read as _;
    use std::time::{Duration, Instant};

    // The master is non-blocking, so a bare `read` returns WouldBlock while
    // the child is still starting; spin briefly and keep draining.
    let mut out = Vec::new();
    let start = Instant::now();
    let mut chunk = [0_u8; 4096];
    loop {
        match master.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => out.extend_from_slice(&chunk[..n]),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                if start.elapsed() >= Duration::from_secs(5) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
                continue;
            }
            Err(_) => break,
        }
    }
    out
}

/// Spawn `/bin/sh -c pwd` in `cwd` and return its output (`None` when no PTY
/// is available — `-c` avoids interactive shells' blocking terminfo queries).
fn spawn_pwd(cwd: Option<&Path>) -> Option<String> {
    let mut cmd = Command::new("/bin/sh");
    cmd.arg("-c").arg("pwd");
    let config = PtyConfig {
        winsize: Winsize {
            ws_row: 24,
            ws_col: 80,
            ws_xpixel: 0,
            ws_ypixel: 0,
        },
        cwd,
        command: None,
    };
    let (master, child) = spawn(&config, cmd).ok()?;
    let bytes = drain_master(std::fs::File::from(master));
    // Reap the shell — `drain_master` returned at its EOF (the shell exited),
    // so this only needs one non-blocking wait to collect the zombie.
    if let Some(pid) = rustix::process::Pid::from_raw(child as i32) {
        let _ = rustix::process::waitpid(
            Some(pid),
            rustix::process::WaitOptions::NOHANG,
        );
    }
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// `resolve_working_directory` unit tests — no PTY needed, so the fallback
/// stays covered when the E2E spawn tests skip in a PTY-less sandbox.
#[test]
fn resolve_working_directory_keeps_a_live_directory() {
    let cwd = std::env::current_dir().unwrap();
    assert_eq!(
        resolve_working_directory(Some(&cwd)).as_deref(),
        Some(cwd.as_path())
    );
}

#[test]
fn resolve_working_directory_falls_back_to_home_for_missing() {
    let Some(home) = std::env::home_dir().filter(|h| h.is_dir()) else {
        eprintln!("no usable $HOME; skipping");
        return;
    };
    let missing = std::env::temp_dir()
        .join(format!("jetfish-missing-{}", std::process::id()));
    assert_eq!(
        resolve_working_directory(Some(&missing)).as_deref(),
        Some(home.as_path())
    );
}

#[test]
fn resolve_working_directory_defaults_to_home() {
    let Some(home) = std::env::home_dir().filter(|h| h.is_dir()) else {
        eprintln!("no usable $HOME; skipping");
        return;
    };
    assert_eq!(
        resolve_working_directory(None).as_deref(),
        Some(home.as_path())
    );
}

/// OSC 7 cwd support: a child spawned with a directory starts there.
#[test]
fn spawn_honors_cwd() {
    let dir = std::env::temp_dir()
        .join(format!("jetfish-cwd-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let Some(text) = spawn_pwd(Some(&dir)) else {
        let _ = std::fs::remove_dir_all(&dir);
        eprintln!("no pseudo-terminal available; skipping");
        return;
    };
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        text.contains("jetfish-cwd"),
        "child did not start in the requested cwd: {text:?}"
    );
}

/// A first window (`cwd = None`) starts in `$HOME`, not the parent's launch
/// directory (which is `/` for a Finder/Dock launch).
#[test]
fn spawn_defaults_to_home_when_no_cwd() {
    let Some(home) = std::env::home_dir().filter(|h| h.is_dir()) else {
        eprintln!("no usable $HOME; skipping");
        return;
    };
    let Some(text) = spawn_pwd(None) else {
        eprintln!("no pseudo-terminal available; skipping");
        return;
    };
    assert!(
        text.contains(home.to_string_lossy().as_ref()),
        "first window did not start in $HOME: {text:?}"
    );
}

/// A deleted OSC 7 directory must fall back to `$HOME`, not fail the spawn.
#[test]
fn spawn_falls_back_to_home_when_cwd_missing() {
    let Some(home) = std::env::home_dir().filter(|h| h.is_dir()) else {
        eprintln!("no usable $HOME; skipping");
        return;
    };
    let missing = std::env::temp_dir()
        .join(format!("jetfish-missing-{}", std::process::id()));
    let Some(text) = spawn_pwd(Some(&missing)) else {
        eprintln!("no pseudo-terminal available; skipping");
        return;
    };
    assert!(
        text.contains(home.to_string_lossy().as_ref()),
        "stale cwd did not fall back to $HOME: {text:?}"
    );
}

/// No locale vars (Finder launch → `LC_CTYPE=C`) would make `pbcopy` garble
/// UTF-8 as `MacRoman`; the override forces UTF-8 but respects an existing one.
#[test]
fn utf8_locale_override_respects_existing_utf8() {
    let none = |s: &str| Some(s.to_string());
    // No locale → force UTF-8.
    assert_eq!(
        utf8_locale_env(None, None, None),
        Some(("LC_ALL", "en_US.UTF-8"))
    );
    // C locale (the macOS default) → force UTF-8.
    assert_eq!(
        utf8_locale_env(None, none("C"), None),
        Some(("LC_ALL", "en_US.UTF-8"))
    );
    // Any UTF-8 locale in any slot → keep it.
    assert_eq!(utf8_locale_env(None, none("C"), none("en_US.UTF-8")), None);
    assert_eq!(utf8_locale_env(None, none("zh_CN.UTF-8"), None), None);
    assert_eq!(utf8_locale_env(none("C.utf8"), None, None), None);
}

/// macOS fix: `LC_CTYPE=C` `pbcopy` re-encodes UTF-8 as `MacRoman`;
/// `LC_ALL=en_US.UTF-8` passes bytes through. Restores the prior clipboard.
#[test]
#[cfg(target_os = "macos")]
fn pbcopy_roundtrips_utf8_only_under_a_utf8_locale() {
    let saved = Command::new("pbpaste")
        .output()
        .ok()
        .map(|o| o.stdout)
        .unwrap_or_default();
    let restore = || {
        let _ = Command::new("pbcopy").stdin(Stdio::piped()).spawn().and_then(
            |mut c| {
                use std::io::Write;
                c.stdin.as_mut().unwrap().write_all(&saved)?;
                c.wait()
            },
        );
    };

    // The nerd branch icon U+E0A0, 3 UTF-8 bytes.
    let input = b"\xee\x82\xa0";
    let run = |envs: &[(&str, &str)]| -> Vec<u8> {
        let mut child = Command::new("pbcopy")
            .envs(envs.iter().copied())
            .stdin(Stdio::piped())
            .spawn()
            .unwrap();
        use std::io::Write;
        child.stdin.as_mut().unwrap().write_all(input).unwrap();
        child.wait().unwrap();
        Command::new("pbpaste").output().map(|o| o.stdout).unwrap_or_default()
    };

    // C locale: the documented mojibake (U+E0A0 → ÓÇ†).
    let garbled = run(&[("LC_CTYPE", "C")]);
    assert_eq!(
        garbled,
        "\u{00d3}\u{00c7}\u{2020}".as_bytes(),
        "LC_CTYPE=C pbcopy must reproduce the MacRoman garble"
    );
    // Forced UTF-8 locale: the exact fix's environment.
    let clean = run(&[("LC_ALL", "en_US.UTF-8")]);
    assert_eq!(
        clean, input,
        "LC_ALL=en_US.UTF-8 pbcopy must round-trip the UTF-8 bytes"
    );
    restore();
}
