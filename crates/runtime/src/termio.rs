// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Calloop-based PTY reader thread: one loop watches every master fd and
//! forwards reads to each engine's channel; registrations cross threads via a
//! `Channel`.

use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock, mpsc};
use std::thread;

use calloop::channel::{Channel, Event as ChannelEvent, Sender};
use calloop::generic::Generic;
use calloop::{EventLoop, Interest, Mode, PostAction};
use rustix::io::{Errno, fcntl_dupfd_cloexec, read};
use snafu::ResultExt as _;

use jetfish_log::log;
use jetfish_protocol::{Result, WakeSlot};

/// A registration request sent from the main thread to the IO thread.
struct Register {
    fd: OwnedFd,
    tx: mpsc::Sender<Vec<u8>>,
    wake: WakeSlot,
    exited: Arc<AtomicBool>,
    /// Engine id for `pty.out` logging.
    id: Option<u64>,
}

/// Sender into the shared IO thread (created lazily on first registration).
static SENDER: OnceLock<Sender<Register>> = OnceLock::new();

/// Read chunk size: captures a full kernel buffer in one `read`.
const PTY_READ_CHUNK: usize = 16 * 1024;

/// Read at most this many chunks per callback so one busy PTY (`yes`) can't
/// starve the others' read sources.
const DRAIN_CHUNK_BUDGET: usize = 16; // 16 × 16 KiB = 256 KiB per window per turn

fn sender() -> Sender<Register> {
    SENDER
        .get_or_init(|| {
            let (tx, rx) = calloop::channel::channel::<Register>();
            if let Err(e) =
                thread::Builder::new().name("termio".into()).spawn(move || {
                    if let Err(e) = run(rx) {
                        log!("termio", "termio thread exited with error: {e}");
                    }
                })
            {
                log!("termio", "failed to spawn termio thread: {e}");
                // The failed spawn dropped `rx`, so `tx` is disconnected: a
                // later `send` fails and `register` reports it instead of
                // blocking forever.
            }
            tx
        })
        .clone()
}

/// The IO thread's event loop: turns registration requests into `Generic`
/// read sources for each PTY master.
fn run(rx: Channel<Register>) -> calloop::Result<()> {
    let mut event_loop: EventLoop<'_, ()> = EventLoop::try_new()?;
    let handle = event_loop.handle();
    let h = handle.clone();
    handle.insert_source(rx, move |event, (), ()| {
        if let ChannelEvent::Msg(req) = event {
            insert_pty(&h, req);
        }
    })?;
    let mut data = ();
    event_loop.run(None, &mut data, |_| {})
}

/// Insert one PTY master as a level-triggered read source.
fn insert_pty(handle: &calloop::LoopHandle<'_, ()>, req: Register) {
    let Register { fd, tx, wake, exited, id } = req;
    let source = Generic::new(fd, Interest::READ, Mode::Level);
    if let Err(e) = handle.insert_source(source, move |_, fd, ()| {
        Ok(drain_master(fd.as_fd(), id, &exited, &tx, &wake))
    }) {
        log!("termio", "failed to insert PTY source eng={id:?}: {e}");
    }
}

/// Drain one PTY master read round and return the source's next action.
fn drain_master(
    fd: BorrowedFd<'_>,
    id: Option<u64>,
    exited: &AtomicBool,
    tx: &mpsc::Sender<Vec<u8>>,
    wake: &WakeSlot,
) -> PostAction {
    let mut buf = [0_u8; PTY_READ_CHUNK];
    let mut woke = false;
    for _ in 0..DRAIN_CHUNK_BUDGET {
        match read(fd, &mut buf) {
            Ok(0) => return mark_exit(id, exited, wake, "EOF"),
            Ok(n) => {
                if let Some(id) = id {
                    log!(
                        "pty.out",
                        "eng={id} {n} bytes: {}",
                        log::escape_bytes(&buf[..n])
                    );
                }
                if tx.send(buf[..n].to_vec()).is_err() {
                    return PostAction::Remove;
                }
                woke = true;
            }
            // Non-blocking fd: nothing left to read this round.
            Err(Errno::AGAIN) => {
                if woke {
                    notify(wake);
                }
                return PostAction::Continue;
            }
            // Linux reports the slave-close hangup as EIO, not EOF(0).
            Err(Errno::IO) => return mark_exit(id, exited, wake, "EIO"),
            Err(e) => {
                log!("pty.err", "eng={id:?} read err={e:?}");
                return PostAction::Remove;
            }
        }
    }
    // Budget exhausted but the fd is still readable: wake once and yield so
    // the other windows' sources get a turn in this poll cycle.
    if woke {
        notify(wake);
    }
    PostAction::Continue
}

/// The shell exited: mark it, wake the host, and drop the source.
fn mark_exit(
    id: Option<u64>,
    exited: &AtomicBool,
    wake: &WakeSlot,
    kind: &str,
) -> PostAction {
    log!("pty.eof", "eng={id:?} {kind} (shell exited)");
    exited.store(true, Ordering::Relaxed);
    notify(wake);
    PostAction::Remove
}

/// Invoke the wake callback if one has been registered (`set_wake`).
fn notify(wake: &WakeSlot) {
    let Ok(guard) = wake.lock() else {
        return;
    };
    let Some(w) = guard.as_ref() else {
        return;
    };
    w();
}

/// Register a PTY master with the reader thread. The master is duplicated;
/// returns the receiver and the exit flag.
pub fn register(
    master: &impl AsFd,
    wake: WakeSlot,
    id: Option<u64>,
) -> Result<(mpsc::Receiver<Vec<u8>>, Arc<AtomicBool>)> {
    let fd = fcntl_dupfd_cloexec(master, 0)
        .whatever_context::<_, jetfish_protocol::Error>(
            "duplicate the PTY master fd",
        )?;
    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    let exited = Arc::new(AtomicBool::new(false));
    sender()
        .send(Register { fd, tx, wake, exited: exited.clone(), id })
        .whatever_context::<_, jetfish_protocol::Error>(
        "register PTY with the reader thread",
    )?;
    Ok((rx, exited))
}

#[cfg(test)]
#[path = "../tests/termio/mod.rs"]
mod tests;
