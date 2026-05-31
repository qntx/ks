//! Best-effort, process-wide hardening that keeps decrypted secrets off disk.
//!
//! [`harden`] is called once at startup. Every measure is defence-in-depth on
//! top of `Zeroizing`/`secrecy`, so failures are ignored rather than fatal.
//!
//! Coverage by platform:
//!
//! - **Unix:** disables core dumps (`RLIMIT_CORE = 0`), locks all current and
//!   future pages into RAM (`mlockall`) so plaintext cannot be paged to swap,
//!   and blocks debugger attachment (Linux `PR_SET_DUMPABLE = 0`, macOS
//!   `PT_DENY_ATTACH` in release builds).
//! - **Windows:** suppresses the Windows Error Reporting crash dialog and fault
//!   dump (`SetErrorMode`). Windows has no portable process-wide page-locking
//!   primitive (only per-region `VirtualLock`), so swap protection is **not**
//!   applied there; this is a documented exemption.
//!
//! All FFI lives in this module so the rest of the workspace stays
//! `#![deny(unsafe_code)]`.
#![allow(
    unsafe_code,
    reason = "process hardening (core-dump, swap, ptrace, crash-dump policy) requires libc / Windows FFI; every call is audited and documented with a SAFETY note"
)]

/// Applies all available hardening for the current platform. Best-effort: any
/// failure is silently ignored.
pub fn harden() {
    #[cfg(unix)]
    unix::harden();
    #[cfg(windows)]
    windows::harden();
}

/// Reads an environment variable and removes it from the process environment,
/// returning its value if it was set and non-empty.
///
/// Clearing a passphrase passed via the environment stops it lingering where a
/// same-user process could read it (`/proc/<pid>/environ`) or where it would be
/// inherited by child processes spawned later (e.g. by `ks run`).
///
/// Must be called while the process is still single-threaded (CLI startup or a
/// command handler before any thread is spawned).
#[must_use]
#[allow(
    clippy::disallowed_methods,
    reason = "clearing a passphrase from the env is the intended security action here, and is safe because the process is single-threaded at call time"
)]
pub fn take_env(name: &str) -> Option<String> {
    let value = std::env::var(name).ok().filter(|v| !v.is_empty());
    // SAFETY: in edition 2024 `remove_var` is `unsafe` because concurrent env
    // mutation is UB. `ks` is single-threaded at the points this is called, so
    // no other thread can read or write the environment concurrently.
    unsafe {
        std::env::remove_var(name);
    }
    value
}

#[cfg(unix)]
mod unix {
    pub fn harden() {
        disable_core_dumps();
        lock_memory();
        deny_debugger();
    }

    /// Sets `RLIMIT_CORE` to zero so a crash cannot write process memory to a
    /// core file.
    fn disable_core_dumps() {
        let limit = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        // SAFETY: `setrlimit` reads the `rlimit` we pass by pointer and sets a
        // kernel limit. It neither retains the pointer nor touches our memory.
        unsafe {
            libc::setrlimit(libc::RLIMIT_CORE, &raw const limit);
        }
    }

    /// Locks resident pages into RAM so decrypted secrets are kept out of swap.
    /// Best-effort and deliberately conservative: `MCL_FUTURE` is requested only
    /// when `RLIMIT_MEMLOCK` is unlimited, because otherwise a later memory-hard
    /// allocation (age's scrypt KDF on unlock) would exceed the limit and the
    /// allocator would abort the whole process.
    fn lock_memory() {
        // SAFETY: each call takes a valid pointer to the local `rlimit` or a
        // scalar flag; the kernel copies the values and retains no pointer.
        unsafe {
            let mut limit = libc::rlimit {
                rlim_cur: 0,
                rlim_max: 0,
            };
            if libc::getrlimit(libc::RLIMIT_MEMLOCK, &raw mut limit) != 0 {
                return;
            }
            // Raise the soft limit to the hard cap so we lock as much as allowed.
            limit.rlim_cur = limit.rlim_max;
            libc::setrlimit(libc::RLIMIT_MEMLOCK, &raw const limit);
            let flags = if limit.rlim_max == libc::RLIM_INFINITY {
                libc::MCL_CURRENT | libc::MCL_FUTURE
            } else {
                libc::MCL_CURRENT
            };
            libc::mlockall(flags);
        }
    }

    #[cfg(target_os = "linux")]
    fn deny_debugger() {
        // PR_SET_DUMPABLE=0 marks the process non-dumpable, which also denies
        // `ptrace` attachment by other non-root processes.
        // SAFETY: `prctl` with `PR_SET_DUMPABLE` takes scalar arguments only.
        unsafe {
            libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0);
        }
    }

    #[cfg(target_os = "macos")]
    fn deny_debugger() {
        // Only in release builds, so developers can still attach a debugger.
        #[cfg(not(debug_assertions))]
        {
            const PT_DENY_ATTACH: libc::c_int = 31;
            // SAFETY: `ptrace(PT_DENY_ATTACH, ...)` takes scalar arguments and a
            // null address; it does not dereference any of our memory.
            unsafe {
                libc::ptrace(PT_DENY_ATTACH, 0, std::ptr::null_mut(), 0);
            }
        }
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    fn deny_debugger() {}
}

#[cfg(windows)]
mod windows {
    use windows_sys::Win32::System::Diagnostics::Debug::{
        SEM_FAILCRITICALERRORS, SEM_NOGPFAULTERRORBOX, SetErrorMode,
    };

    pub fn harden() {
        // Suppress the WER crash dialog and the general-protection-fault dump so
        // a fault cannot persist decrypted memory to disk.
        // SAFETY: `SetErrorMode` takes a scalar flag and returns the previous
        // mode; it does not touch our memory.
        unsafe {
            SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX);
        }
    }
}
