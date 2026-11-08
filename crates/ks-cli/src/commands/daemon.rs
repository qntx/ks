//! `ks __clipboard <secs>` — the hidden clipboard daemon.

use crate::output::CliResult;

/// Runs the daemon; returns its exit code.
pub fn run_daemon(secs: u64) -> CliResult<i32> {
    Ok(crate::clipboard::daemon(secs))
}
