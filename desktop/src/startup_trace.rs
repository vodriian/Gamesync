//! Opt-in startup and idle timing. `GAMESYNC_STARTUP_TRACE=1` prints one line
//! per phase to stderr. Nothing is printed otherwise.
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        OnceLock,
    },
    time::Instant,
};

static START: OnceLock<Option<Instant>> = OnceLock::new();

fn start() -> Option<Instant> {
    *START.get_or_init(|| std::env::var_os("GAMESYNC_STARTUP_TRACE").map(|_| Instant::now()))
}

pub fn enabled() -> bool {
    start().is_some()
}

/// Milliseconds since the first mark. Call once at the top of `main`, so later
/// marks count from process entry.
pub fn mark(label: &str) {
    if let Some(start) = start() {
        eprintln!(
            "[startup] {label} ms={:.1}",
            start.elapsed().as_secs_f64() * 1000.
        );
    }
}

/// A mark for a point that repeats, such as a render. Only the first call prints.
pub fn mark_once(seen: &AtomicBool, label: &str) {
    if enabled() && !seen.swap(true, Ordering::Relaxed) {
        mark(label);
    }
}

/// One background refresh: the folder scan, the sync round, and whether the
/// result replaced the displayed library.
pub fn idle(scan_ms: f64, sync_ms: f64, applied: bool) {
    if enabled() {
        eprintln!("[idle] scan_ms={scan_ms:.1} sync_ms={sync_ms:.1} applied={applied}");
    }
}
