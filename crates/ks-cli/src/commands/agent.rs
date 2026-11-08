//! `ks __agent` — the session agent daemon (hidden, spawned by `ks unlock`).

use std::io::Read as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use interprocess::local_socket::{
    Listener, ListenerNonblockingMode, ListenerOptions,
    traits::{Listener as _, Stream as _, StreamCommon as _},
};
use ks::ErrorCode;
use ks::agent::{ConnectionEnd, Handler};
use zeroize::Zeroizing;

use super::{Cli, CliResult, Out, err, fail, vault};

/// Unix: reads the vault key from stdin (to EOF), applies hardening, binds
/// the endpoint, prints `ready`, then serves connections until idle/lifetime
/// expiry or a `lock` request.
pub(crate) fn run(cli: &Cli, _out: &Out, ttl: u64, max: u64) -> CliResult<i32> {
    let v = vault(cli)?;

    let mut text = Zeroizing::new(String::new());
    std::io::stdin()
        .read_to_string(&mut text)
        .map_err(|e| (ErrorCode::Io, format!("cannot read agent identity: {e}")))?;
    let handler = Arc::new(
        Handler::new(age::secrecy::SecretString::from(std::mem::take(&mut *text)))
            .map_err(|e| err(&e))?,
    );
    harden_agent();
    let ep = ks::agent::endpoint(&v).map_err(|e| err(&e))?;

    // Stale-socket rule: a connect+hello proves a live agent -> refuse to
    // start; anything else is a leftover file to reclaim.
    if ks::agent::connect_raw(&v).map_err(|e| err(&e))?.is_some() {
        return fail(ErrorCode::Unavailable, "another agent is already running");
    }
    #[cfg(unix)]
    if let Some(path) = ep_path(&ep) {
        drop(std::fs::remove_file(path));
    }

    let name = ep.socket_name().map_err(|e| err(&e))?;
    let options = ListenerOptions::new().name(name).reclaim_name(false);
    #[cfg(windows)]
    {
        use interprocess::os::windows::security_descriptor::SecurityDescriptor;
        // SDDL: protected DACL, GENERIC_ALL for the object owner only.
        let sd = SecurityDescriptor::deserialize(
            &widestring::U16CString::from_str("D:P(A;;GA;;;OW)")
                .map_err(|e| (ErrorCode::Internal, format!("bad SDDL: {e}")))?,
        )
        .map_err(|e| (ErrorCode::Internal, format!("bad security descriptor: {e}")))?;
        let options = options.security_descriptor(sd);
    }
    let listener = options.create_sync_as::<Listener>().map_err(|e| {
        (
            ErrorCode::Unavailable,
            format!("cannot bind agent endpoint: {e}"),
        )
    })?;
    #[cfg(unix)]
    if let Some(path) = ep_path(&ep) {
        use std::os::unix::fs::PermissionsExt as _;
        drop(std::fs::set_permissions(
            path,
            std::fs::Permissions::from_mode(0o600),
        ));
    }

    // Signal ks unlock, then stop talking to the terminal.
    {
        use std::io::Write as _;
        drop(writeln!(std::io::stdout(), "ready"));
        drop(std::io::stdout().flush());
    }

    let stop = Arc::new(AtomicBool::new(false));
    let last_activity = Arc::new(AtomicU64::new(now_secs()));
    let born = Instant::now();

    std::thread::spawn({
        let stop = Arc::clone(&stop);
        let last = Arc::clone(&last_activity);
        move || watchdog(&stop, &last, born, ttl, max)
    });

    listener
        .set_nonblocking(ListenerNonblockingMode::Accept)
        .map_err(|e| {
            (
                ErrorCode::Unavailable,
                format!("cannot nonblock listener: {e}"),
            )
        })?;

    while !stop.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok(stream) => {
                if !same_user(&stream) {
                    continue;
                }
                let handler = Arc::clone(&handler);
                let stop = Arc::clone(&stop);
                let last = Arc::clone(&last_activity);
                std::thread::spawn(move || serve(stream, &handler, &stop, &last, ttl, born, max));
            }
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::ConnectionAborted
                ) =>
            {
                sleep(Duration::from_millis(50));
            }
            Err(_) => break,
        }
    }

    ep.cleanup();
    Ok(0)
}

/// Idle/lifetime watchdog: flips `stop` when either limit is reached.
fn watchdog(stop: &AtomicBool, last: &AtomicU64, born: Instant, ttl: u64, max: u64) {
    while !stop.load(Ordering::Relaxed) {
        let idle = now_secs().saturating_sub(last.load(Ordering::Relaxed));
        if idle >= ttl || born.elapsed().as_secs() >= max {
            stop.store(true, Ordering::Relaxed);
            return;
        }
        sleep(Duration::from_millis(250));
    }
}

/// Blocking sleep inside the daemon (no async runtime in the agent).
#[allow(
    clippy::disallowed_methods,
    reason = "the agent is a blocking sync daemon; a short poll sleep is the whole loop"
)]
fn sleep(d: Duration) {
    std::thread::sleep(d);
}

/// Serves one accepted connection on its own thread.
fn serve(
    stream: interprocess::local_socket::Stream,
    handler: &Handler,
    stop: &AtomicBool,
    last: &AtomicU64,
    ttl: u64,
    born: Instant,
    max: u64,
) {
    // Streams inherit the listener's nonblocking flag on unix; the framed
    // protocol needs plain blocking I/O.
    let _reverted = stream.set_nonblocking(false);
    let end = handler.serve_connection(
        stream,
        &|| last.store(now_secs(), Ordering::Relaxed),
        &|| idle_left(last, ttl),
        &|| life_left(&born, max),
    );
    if matches!(end, Ok(ConnectionEnd::LockRequested)) {
        stop.store(true, Ordering::Relaxed);
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn idle_left(last: &AtomicU64, ttl: u64) -> u64 {
    ttl.saturating_sub(now_secs().saturating_sub(last.load(Ordering::Relaxed)))
}

fn life_left(born: &Instant, max: u64) -> u64 {
    max.saturating_sub(born.elapsed().as_secs())
}

#[cfg(unix)]
fn ep_path(ep: &ks::agent::Endpoint) -> Option<&std::path::Path> {
    ep.path()
}

/// Same-user check: the peer's euid must equal ours.
#[cfg(unix)]
fn same_user(stream: &interprocess::local_socket::Stream) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    let Ok(creds) = stream.peer_creds() else {
        return false;
    };
    let Some(euid) = creds.euid() else {
        return false;
    };
    // A file we create carries our uid; avoids libc/unsafe for getuid.
    let Ok(dir) = tempfile::tempdir() else {
        return false;
    };
    std::fs::metadata(dir.path()).is_ok_and(|m| m.uid() == euid)
}

/// Windows relies on the listener's owner-only security descriptor.
#[cfg(windows)]
fn same_user(_stream: &interprocess::local_socket::Stream) -> bool {
    true
}

/// Agent-specific hardening on top of the startup hardening.
#[cfg(all(unix, not(target_os = "linux"), not(target_os = "macos")))]
#[allow(unsafe_code, reason = "setsid is a syscall")]
fn harden_agent() {
    // SAFETY: setsid detaches the daemon from its controlling terminal.
    drop(unsafe { libc::setsid() });
}

#[cfg(target_os = "linux")]
#[allow(
    unsafe_code,
    reason = "process hardening syscalls; each call is a single op with a SAFETY note"
)]
fn harden_agent() {
    // SAFETY: setsid detaches the daemon from its controlling terminal.
    drop(unsafe { libc::setsid() });
    // SAFETY: mlockall pins current and future memory; best-effort, and the
    // calls touch no memory themselves.
    unsafe {
        libc::mlockall(libc::MCL_CURRENT | libc::MCL_FUTURE);
    }
}

#[cfg(target_os = "macos")]
#[allow(
    unsafe_code,
    reason = "process hardening syscalls; each call is a single op with a SAFETY note"
)]
fn harden_agent() {
    // SAFETY: setsid detaches the daemon from its controlling terminal.
    unsafe {
        libc::setsid();
    }
    // SAFETY: ptrace(PT_DENY_ATTACH) asks the kernel to refuse debugger
    // attachment to this process; no memory is dereferenced.
    unsafe {
        libc::ptrace(libc::PT_DENY_ATTACH, 0, std::ptr::null_mut(), 0);
    }
}

#[cfg(windows)]
fn harden_agent() {}
