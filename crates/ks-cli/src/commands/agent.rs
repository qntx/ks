//! `ks __agent` — the session agent daemon (hidden, spawned by `ks unlock`).

use std::io::Read as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

#[cfg(unix)]
use interprocess::local_socket::traits::StreamCommon as _;
use interprocess::local_socket::{Listener, ListenerOptions, traits::Listener as _};
#[cfg(windows)]
use interprocess::os::windows::local_socket::ListenerOptionsExt as _;
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
    crate::harden::agent();
    let ep = ks::agent::ensure_endpoint(&v).map_err(|e| err(&e))?;

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
    let options = options.security_descriptor(owner_only_dacl()?);
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

    let own_uid = own_uid();
    let wake = Arc::new({
        let root = v.root().to_path_buf();
        move || {
            if let Ok(v2) = ks::Vault::open(&root) {
                drop(ks::agent::connect_raw(&v2));
            }
        }
    });

    std::thread::spawn({
        let stop = Arc::clone(&stop);
        let last = Arc::clone(&last_activity);
        let wake = Arc::clone(&wake);
        move || watchdog(&stop, &last, born, ttl, max, &*wake)
    });

    // Blocking accept; stop events wake it with a throwaway self-connect.
    loop {
        match listener.accept() {
            Ok(_wake) if stop.load(Ordering::Relaxed) => break,
            Ok(stream) => {
                if !same_user(&stream, own_uid) {
                    continue;
                }
                let handler = Arc::clone(&handler);
                let stop = Arc::clone(&stop);
                let last = Arc::clone(&last_activity);
                let wake = Arc::clone(&wake);
                std::thread::spawn(move || {
                    serve(
                        stream,
                        &ConnCtx {
                            handler: &handler,
                            stop: &stop,
                            last: &last,
                            ttl,
                            born,
                            max,
                            wake: &*wake,
                        },
                    );
                });
            }
            Err(e) if e.kind() == std::io::ErrorKind::ConnectionAborted => {}
            Err(_) => break,
        }
    }

    ep.cleanup();
    Ok(0)
}

/// Idle/lifetime watchdog: flips `stop` when either limit is reached, then
/// wakes the blocked accept with a throwaway self-connect.
fn watchdog(
    stop: &AtomicBool,
    last: &AtomicU64,
    born: Instant,
    ttl: u64,
    max: u64,
    wake: &(dyn Fn() + Send + Sync),
) {
    while !stop.load(Ordering::Relaxed) {
        let idle = now_secs().saturating_sub(last.load(Ordering::Relaxed));
        if idle >= ttl || born.elapsed().as_secs() >= max {
            stop.store(true, Ordering::Relaxed);
            wake();
            return;
        }
        sleep_250();
    }
}

/// 250 ms poll for the watchdog — cheap and not in the command path.
#[allow(
    clippy::disallowed_methods,
    reason = "blocking sync daemon; the watchdog polls outside the request path"
)]
fn sleep_250() {
    std::thread::sleep(Duration::from_millis(250));
}

/// Per-connection state shared with `serve`.
struct ConnCtx<'a> {
    handler: &'a Handler,
    stop: &'a AtomicBool,
    last: &'a AtomicU64,
    ttl: u64,
    born: Instant,
    max: u64,
    wake: &'a (dyn Fn() + Send + Sync),
}

/// Serves one accepted connection on its own thread. A `lock` request stops
/// the agent and wakes the blocked accept with a throwaway self-connect.
fn serve(stream: interprocess::local_socket::Stream, ctx: &ConnCtx<'_>) {
    let end = ctx.handler.serve_connection(
        stream,
        &|| ctx.last.store(now_secs(), Ordering::Relaxed),
        &|| idle_left(ctx.last, ctx.ttl),
        &|| life_left(&ctx.born, ctx.max),
    );
    if matches!(end, Ok(ConnectionEnd::LockRequested)) {
        ctx.stop.store(true, Ordering::Relaxed);
        (ctx.wake)();
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

#[cfg(unix)]
fn own_uid() -> u32 {
    rustix::process::getuid().as_raw()
}

#[cfg(windows)]
const fn own_uid() -> u32 {
    0
}

/// Same-user check: the peer's euid must equal `own_uid`.
#[cfg(unix)]
fn same_user(stream: &interprocess::local_socket::Stream, own_uid: u32) -> bool {
    stream.peer_creds().ok().and_then(|c| c.euid()) == Some(own_uid)
}

/// Builds the SDDL `D:P(A;;GA;;;OW)` descriptor: a protected DACL granting
/// `GENERIC_ALL` to the pipe object's owner (the current user) only.
#[cfg(windows)]
fn owner_only_dacl() -> CliResult<interprocess::os::windows::security_descriptor::SecurityDescriptor>
{
    use interprocess::os::windows::security_descriptor::SecurityDescriptor;
    let sddl = widestring::U16CString::from_str("D:P(A;;GA;;;OW)")
        .map_err(|e| (ErrorCode::Internal, format!("bad SDDL: {e}")))?;
    SecurityDescriptor::deserialize(&sddl)
        .map_err(|e| (ErrorCode::Internal, format!("bad security descriptor: {e}")))
}

/// Windows relies on the listener's owner-only security descriptor.
#[cfg(windows)]
const fn same_user(_stream: &interprocess::local_socket::Stream, _own_uid: u32) -> bool {
    true
}
