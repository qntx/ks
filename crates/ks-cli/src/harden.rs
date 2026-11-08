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
const fn harden_platform() {}

/// Extra hardening for the `ks __agent` daemon: detach the session, and on
/// Linux pin memory, on macOS refuse debugger attachment. Best-effort.
#[cfg(unix)]
#[allow(
    unsafe_code,
    reason = "process hardening syscalls; one unsafe op per block with SAFETY notes"
)]
pub(crate) fn agent() {
    let _sid = rustix::process::setsid();
    #[cfg(target_os = "linux")]
    // SAFETY: mlockall pins current and future pages; it touches no memory
    // itself and its failure is non-fatal.
    unsafe {
        libc::mlockall(libc::MCL_CURRENT | libc::MCL_FUTURE);
    }
    #[cfg(target_os = "macos")]
    // SAFETY: ptrace(PT_DENY_ATTACH) asks the kernel to refuse debugger
    // attachment to this process; no memory is dereferenced.
    unsafe {
        libc::ptrace(libc::PT_DENY_ATTACH, 0, std::ptr::null_mut(), 0);
    }
}

/// Windows has no equivalent of setsid/ptrace hardening here; the daemon is
/// already spawned detached.
#[cfg(windows)]
pub(crate) const fn agent() {}

/// Windows `CreateProcess` inherits every inheritable handle, so a detached
/// child would keep the parent's stdout/stderr pipes open and the caller
/// would wait for an EOF that only comes when the child exits. Clears
/// `HANDLE_FLAG_INHERIT` on our std handles; a no-op elsewhere (unix is
/// CLOEXEC). The parent's own stdio keeps working — the flag only affects
/// inheritance.
#[cfg(windows)]
#[allow(
    unsafe_code,
    reason = "Win32 handle flag updates; one unsafe op per block with SAFETY notes"
)]
pub(crate) fn no_inherit_std_handles() {
    use windows_sys::Win32::Foundation::{
        HANDLE_FLAG_INHERIT, INVALID_HANDLE_VALUE, SetHandleInformation,
    };
    use windows_sys::Win32::System::Console::{
        GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
    };
    for id in [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
        // SAFETY: GetStdHandle takes a constant id and touches no memory.
        let handle = unsafe { GetStdHandle(id) };
        if handle.is_null() || handle == INVALID_HANDLE_VALUE {
            continue;
        }
        // SAFETY: `handle` is a valid std handle; clearing HANDLE_FLAG_INHERIT
        // is a documented SetHandleInformation operation.
        // Return value ignored: the operation is best-effort.
        let _cleared = unsafe { SetHandleInformation(handle, HANDLE_FLAG_INHERIT, 0) };
    }
}

/// Non-windows platforms close all inheritable handles on exec (CLOEXEC).
#[cfg(not(windows))]
pub(crate) const fn no_inherit_std_handles() {}
