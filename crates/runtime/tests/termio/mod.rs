// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use rustix::termios::Winsize;

use super::*;
use crate::pty::{PtyConfig, spawn};

/// Accumulate chunks from `rx` until `needle` appears, the channel
/// disconnects (shell exited), or the deadline passes.
fn collect_until(
    rx: &mpsc::Receiver<Vec<u8>>,
    needle: &str,
    timeout: Duration,
) -> String {
    let deadline = Instant::now() + timeout;
    let mut out = Vec::new();
    loop {
        if String::from_utf8_lossy(&out).contains(needle) {
            break;
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        match rx.recv_timeout(remaining) {
            Ok(chunk) => out.extend_from_slice(&chunk),
            Err(mpsc::RecvTimeoutError::Timeout)
            | Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Multiple windows share one termio reader thread; each must read only its
/// own PTY, write to its own shell, and flip its own `exited` flag.
#[test]
fn multiple_windows_write_and_read_independently() {
    let n = 3;
    let mut windows = Vec::new();
    for i in 0..n {
        // Each shell echoes a unique tag, then reads one line and echoes
        // it back prefixed by that tag, then exits.
        let mut cmd = std::process::Command::new("/bin/sh");
        cmd.arg("-c").arg(format!("echo w{i}; IFS= read -r x; echo w{i}:$x"));
        let config = PtyConfig {
            winsize: Winsize {
                ws_row: 24,
                ws_col: 80,
                ws_xpixel: 0,
                ws_ypixel: 0,
            },
            cwd: None,
            command: None,
        };
        let Ok((master, child)) = spawn(&config, cmd) else {
            eprintln!("no pseudo-terminal available; skipping");
            return;
        };
        let (rx, exited) = register(
            &master,
            Arc::new(Mutex::new(None)),
            Some(u64::try_from(i).unwrap()),
        )
        .unwrap();
        windows.push((master, child, rx, exited, i));
    }

    // 1. Each window's startup echo carries only its own tag.
    for (_, _, rx, _, i) in &windows {
        let out = collect_until(rx, &format!("w{i}"), Duration::from_secs(5));
        assert!(
            out.contains(&format!("w{i}")),
            "window {i} did not see its startup echo: {out:?}"
        );
        for j in 0..n {
            if j != *i {
                assert!(
                    !out.contains(&format!("w{j}")),
                    "window {i} saw window {j}'s output: {out:?}"
                );
            }
        }
    }

    // 2. Write a unique line to each master; the shell echoes it back
    //    with its own tag, proving the write/read path is per-window.
    for (master, _, rx, _, i) in &mut windows {
        rustix::io::write(&*master, format!("hello-{i}\n").as_bytes())
            .unwrap();
        let out = collect_until(
            rx,
            &format!("w{i}:hello-{i}"),
            Duration::from_secs(5),
        );
        assert!(
            out.contains(&format!("w{i}:hello-{i}")),
            "window {i} did not echo its input: {out:?}"
        );
    }

    // 3. After the read+echo each shell exits; its exit flag flips
    //    independently without disturbing the others.
    let deadline = Instant::now() + Duration::from_secs(5);
    for (_, child, _, exited, i) in &windows {
        while !exited.load(Ordering::Relaxed) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(
            exited.load(Ordering::Relaxed),
            "window {i} exit was not observed"
        );
        // Reap the zombie so the test leaves no defunct children.
        let _ = rustix::process::waitpid(
            rustix::process::Pid::from_raw(*child as i32),
            rustix::process::WaitOptions::NOHANG,
        );
    }
}

/// A busy window (an infinite producer) must not starve the others sharing
/// the termio reader: the drain yields after [`DRAIN_CHUNK_BUDGET`] chunks.
#[test]
fn busy_window_does_not_block_other_windows_reads() {
    // A: `yes` streams forever (never hits EAGAIN).
    let mut a = std::process::Command::new("/bin/sh");
    a.arg("-c").arg("yes");
    let config = PtyConfig {
        winsize: Winsize {
            ws_row: 24,
            ws_col: 80,
            ws_xpixel: 0,
            ws_ypixel: 0,
        },
        cwd: None,
        command: None,
    };
    let (master_a, child_a) = spawn(&config, a).unwrap();
    let (rx_a, _) =
        register(&master_a, Arc::new(Mutex::new(None)), Some(0)).unwrap();

    // B: one-shot marker.
    let mut b = std::process::Command::new("/bin/sh");
    b.arg("-c").arg("echo markerB");
    let (master_b, child_b) = spawn(&config, b).unwrap();
    let (rx_b, _) =
        register(&master_b, Arc::new(Mutex::new(None)), Some(1)).unwrap();

    // Poll both: drain A so its unbounded channel does not grow without
    // bound, and watch for B's marker. B must arrive promptly even
    // though A never stops writing.
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut b_out = Vec::new();
    loop {
        while rx_a.try_recv().is_ok() {}
        match rx_b.try_recv() {
            Ok(chunk) => {
                b_out.extend_from_slice(&chunk);
                if String::from_utf8_lossy(&b_out).contains("markerB") {
                    break;
                }
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => break,
        }
        if Instant::now() >= deadline {
            break;
        }
        thread::sleep(Duration::from_millis(5));
    }

    let b_text = String::from_utf8_lossy(&b_out).into_owned();
    assert!(
        b_text.contains("markerB"),
        "window B was starved by window A: {b_text:?}"
    );

    // Clean up: kill A's session (the `yes` child lives in the shell's
    // process group), then reap both direct children.
    let _ = rustix::process::kill_process_group(
        rustix::process::Pid::from_raw(child_a as i32).unwrap(),
        rustix::process::Signal::KILL,
    );
    for child in [child_a, child_b] {
        let _ = rustix::process::waitpid(
            rustix::process::Pid::from_raw(child as i32),
            rustix::process::WaitOptions::NOHANG,
        );
    }
}
