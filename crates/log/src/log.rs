// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Lightweight PTY / layout logging, gated by `JETFISH_LOG`.
//!
//! - `JETFISH_LOG=1`              → logs to stderr.
//! - `JETFISH_LOG=/path/to/file`  → logs to that file (created/truncated).
//! - unset / empty / `0`          → disabled.

use std::fmt::Write as _;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use compact_str::CompactString;

// fastrace reporter: reuses the log sink, no allocation when off.
struct GlyphReporter;

impl fastrace::collector::Reporter for GlyphReporter {
    fn report(&mut self, spans: Vec<fastrace::collector::SpanRecord>) {
        if !enabled() {
            return;
        }
        for span in &spans {
            let dur_ms = span.duration_ns as f64 / 1_000_000.0;
            let mut line = format!("span {} dur={:.2}ms", span.name, dur_ms);
            for (k, v) in &span.properties {
                let _ = write!(line, " {k}={v}");
            }
            log("trace", &line);
            for ev in &span.events {
                let mut eline = format!("  event {}", ev.name);
                for (k, v) in &ev.properties {
                    let _ = write!(eline, " {k}={v}");
                }
                log("trace", &eline);
            }
        }
    }
}

/// A root-span guard, reported on drop. Type-erased because
/// `fastrace::LocalParentGuard` is not nameable.
#[allow(
    missing_debug_implementations,
    reason = "the guard wraps a type-erased fastrace span"
)]
pub struct TraceFrame {
    _guard: Option<Box<dyn std::any::Any>>,
    _root: Option<fastrace::Span>,
}

static ENABLED: AtomicBool = AtomicBool::new(false);
static WRITER: OnceLock<Mutex<Box<dyn Write + Send>>> = OnceLock::new();
static START: OnceLock<Instant> = OnceLock::new();

/// Initialize the log sink from `JETFISH_LOG` (idempotent).
pub fn init() {
    let raw = std::env::var("JETFISH_LOG").unwrap_or_default();
    let enabled = !raw.is_empty() && raw != "0";
    ENABLED.store(enabled, Ordering::Relaxed);
    if !enabled {
        return;
    }

    START.get_or_init(Instant::now);
    let writer: Box<dyn Write + Send> = if raw == "1" {
        Box::new(std::io::stderr())
    } else {
        match std::fs::File::create(&raw) {
            Ok(f) => {
                eprintln!("jetfish log: writing to {raw}");
                Box::new(std::io::BufWriter::new(f))
            }
            Err(e) => {
                eprintln!(
                    "jetfish log: cannot open {raw}: {e}; falling back to stderr"
                );
                Box::new(std::io::stderr())
            }
        }
    };
    let _ = WRITER.set(Mutex::new(writer));

    // fastrace shares the sink: spans/events land here instead of scattered
    // `eprintln!`.
    fastrace::set_reporter(
        GlyphReporter,
        fastrace::collector::Config::default()
            .report_interval(std::time::Duration::from_millis(50)),
    );
}

/// Whether PTY logging is active.
#[inline]
pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// Milliseconds since logging was first enabled.
fn elapsed_ms() -> u128 {
    START.get_or_init(Instant::now).elapsed().as_millis()
}

/// Record a fastrace event on the current span; no-op when logging is off.
#[inline]
pub fn trace_event(scope: &'static str, msg: String) {
    fastrace::prelude::LocalSpan::add_event(
        fastrace::prelude::Event::new(scope).with_property(|| ("msg", msg)),
    );
}

/// Begin a root span for one frame/iteration (no-op when tracing is off).
#[must_use]
pub fn begin_trace_frame(name: &'static str) -> TraceFrame {
    if enabled() {
        let root = fastrace::Span::root(
            name,
            fastrace::prelude::SpanContext::random(),
        );
        let guard = root.set_local_parent();
        TraceFrame {
            // `_guard` is declared first so it drops before `_root` (struct
            // fields drop in declaration order), popping the local parent
            // before the root span finishes.
            _guard: Some(Box::new(guard)),
            _root: Some(root),
        }
    } else {
        TraceFrame { _guard: None, _root: None }
    }
}

/// Render a byte slice as a compact, printable/escaped string.
#[must_use]
#[inline]
pub fn escape_bytes(bytes: &[u8]) -> CompactString {
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        match b {
            0x1b => s.push_str("\\x1b"),
            b'\r' => s.push_str("\\r"),
            b'\n' => s.push_str("\\n"),
            b'\t' => s.push_str("\\t"),
            0x07 => s.push_str("\\a"),
            0x00 => s.push_str("\\0"),
            b'\\' => s.push_str("\\\\"),
            0x20..=0x7e => s.push(b as char),
            _ => {
                let _ = write!(s, "\\x{b:02x}");
            }
        }
    }
    s.into()
}

/// Log a line (only when enabled).
#[inline]
pub fn log(scope: &str, msg: &str) {
    if enabled()
        && let Some(m) = WRITER.get()
        && let Ok(mut w) = m.lock()
    {
        let _ = writeln!(w, "[{:>8}ms] {} {}", elapsed_ms(), scope, msg);
        let _ = w.flush();
    }
}

/// Record a scoped log line when [`enabled`] is on. No-op otherwise.
#[macro_export]
macro_rules! log {
    ($scope:expr, $($arg:tt)*) => {
        if $crate::log::enabled() {
            $crate::log::log($scope, &format!($($arg)*));
        }
    };
}

/// A [`log!`]-style macro backed by fastrace events; no-op unless
/// logging is on.
#[macro_export]
macro_rules! trace {
    ($scope:expr, $($arg:tt)*) => {
        if $crate::log::enabled() {
            $crate::log::trace_event($scope, format!($($arg)*));
        }
    };
}
