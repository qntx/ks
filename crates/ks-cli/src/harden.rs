//! Process hardening and the `KS_PASSPHRASE` handoff. The only `unsafe` code
//! in the CLI lives here.

use std::sync::OnceLock;

use age::secrecy::SecretString;

static CAPTURED: OnceLock<Option<SecretString>> = OnceLock::new();

/// Runs at the very start of `main`, before any thread is spawned: captures
/// `KS_PASSPHRASE` into memory, scrubs it from the process environment, and
/// applies platform hardening. All failures are silent best-effort.
#[allow(
    unsafe_code,
    clippy::disallowed_methods,
    reason = "scrubbing KS_PASSPHRASE requires env mutation before threads spawn"
)]
pub(crate) fn run() {
    let captured = std::env::var("KS_PASSPHRASE").ok().map(SecretString::from);
    if captured.is_some() {
        // SAFETY: called before any other thread exists in this process.
        unsafe { std::env::remove_var("KS_PASSPHRASE") };
    }
    CAPTURED.set(captured).ok();
    harden_platform();
}

/// The passphrase captured from `KS_PASSPHRASE` at startup (if any).
pub(crate) fn captured_passphrase() -> Option<SecretString> {
    CAPTURED.get().and_then(Option::clone)
}

#[cfg(unix)]
#[allow(
    unsafe_code,
    reason = "setrlimit/prctl require libc; single-op blocks with SAFETY notes"
)]
fn harden_platform() {
    let limit = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    // SAFETY: `limit` is a valid, initialized `rlimit` that outlives the call;
    // setrlimit only reads it.
    unsafe { libc::setrlimit(libc::RLIMIT_CORE, &raw const limit) };
    // SAFETY: PR_SET_DUMPABLE takes integer arguments only and touches no
    // memory owned by this process.
    #[cfg(target_os = "linux")]
    unsafe {
        libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0)
    };
}

#[cfg(not(unix))]
fn harden_platform() {}
