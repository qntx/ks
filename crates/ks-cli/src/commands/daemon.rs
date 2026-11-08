//! `ks __clipboard <secs>` — the hidden clipboard daemon.

/// Runs the daemon; returns its exit code.
pub(crate) fn run_daemon(secs: u64) -> i32 {
    crate::clipboard::daemon(secs)
}
