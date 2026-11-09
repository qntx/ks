//! The session agent server lifecycle (`feature = "agent-server"`):
//! bind, stale-socket rule, blocking accept with per-connection threads,
//! peer uid check, idle/max timers, wake-up stop, endpoint cleanup.
//! Process hardening stays in the caller's binary.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[cfg(unix)]
use interprocess::local_socket::traits::StreamCommon as _;
use interprocess::local_socket::{Listener, ListenerOptions, traits::Listener as _};
#[cfg(windows)]
use interprocess::os::windows::local_socket::ListenerOptionsExt as _;

use age::secrecy::SecretString;

use crate::agent::{AgentStatus, ConnectionEnd, Endpoint, Handler, connect_raw, ensure_endpoint};
use crate::{Error, Result, Vault};

/// Who serves the agent endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Host {
    /// The `ks` CLI (`ks unlock` / `ks __agent`).
    Cli,
    /// The desktop application.
    Desktop,
}

impl Host {
    /// The value sent in the `hello` response.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cli => "cli",
            Self::Desktop => "desktop",
        }
    }
}

/// Limits and identity of a [`Server`].
#[derive(Debug, Clone, Copy)]
pub struct ServerConfig {
    /// Idle timeout reset only by unwrap/wrap traffic.
    pub ttl: Duration,
    /// Hard lifetime cap counted from `start`.
    pub max: Duration,
    /// Who is serving (sent in `hello`).
    pub host: Host,
}

/// Why [`Server::wait`] returned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    /// No activity for `ttl`.
    Idle,
    /// `max` lifetime reached.
    MaxLifetime,
    /// A client sent `lock`.
    LockRequested,
    /// [`Server::stop`] or [`StopHandle::stop`] was called.
    Stopped,
}

/// A cloneable handle that stops the server without consuming it — a
/// GUI keeps one while a worker thread calls [`Server::wait`].
#[derive(Clone)]
#[allow(
    missing_debug_implementations,
    reason = "holds Arc<dyn Fn> which has no Debug; the handle is opaque"
)]
pub struct StopHandle {
    stop: Arc<AtomicBool>,
    wake: Arc<dyn Fn() + Send + Sync>,
}

impl StopHandle {
    /// Stops the accept loop; [`Server::wait`] returns
    /// [`StopReason::Stopped`]. Idempotent.
    pub fn stop(&self) {
        if !self.stop.swap(true, Ordering::Relaxed) {
            (self.wake)();
        }
    }
}

type Reason = Arc<Mutex<Option<StopReason>>>;

/// A running agent server. [`Server::wait`] blocks until expiry, a
/// `lock` request or `stop`; [`Server::stopper`] yields a handle to
/// stop it from another thread.
#[allow(
    missing_debug_implementations,
    reason = "holds JoinHandle/Arc<dyn Fn> without Debug; the server is opaque"
)]
pub struct Server {
    stop: Arc<AtomicBool>,
    wake: Arc<dyn Fn() + Send + Sync>,
    reason: Reason,
    join: Option<std::thread::JoinHandle<()>>,
    endpoint: Endpoint,
    host: Host,
    born: Instant,
    last: Arc<AtomicU64>,
    ttl: u64,
    max: u64,
}

impl Server {
    /// Binds the endpoint and starts accepting; returns once listening.
    ///
    /// # Errors
    ///
    /// [`Error::Unavailable`] when another agent already answers or the
    /// endpoint cannot be bound; [`Error::Corrupt`] on a malformed
    /// identity; [`Error::Io`] on filesystem failures.
    pub fn start(vault: &Vault, identity: SecretString, config: ServerConfig) -> Result<Self> {
        let handler = Arc::new(Handler::with_host(identity, config.host.as_str())?);
        let ep = ensure_endpoint(vault)?;

        // Stale-socket rule: a connect+hello proves a live agent → refuse;
        // anything else is a leftover file to reclaim.
        if connect_raw(vault)?.is_some() {
            return Err(Error::Unavailable(
                "another agent is already running".into(),
            ));
        }
        #[cfg(unix)]
        if let Some(path) = ep.path() {
            drop(std::fs::remove_file(path));
        }

        let name = ep.socket_name()?;
        let options = ListenerOptions::new().name(name).reclaim_name(false);
        #[cfg(windows)]
        let options = options.security_descriptor(owner_only_dacl()?);
        let listener = options
            .create_sync_as::<Listener>()
            .map_err(|e| Error::Unavailable(format!("cannot bind agent endpoint: {e}")))?;
        #[cfg(unix)]
        if let Some(path) = ep.path() {
            use std::os::unix::fs::PermissionsExt as _;
            drop(std::fs::set_permissions(
                path,
                std::fs::Permissions::from_mode(0o600),
            ));
        }

        let stop = Arc::new(AtomicBool::new(false));
        let last = Arc::new(AtomicU64::new(now_secs()));
        let reason: Reason = Arc::new(Mutex::new(None));
        let born = Instant::now();
        let ttl = config.ttl.as_secs();
        let max = config.max.as_secs();
        let root = vault.root().to_path_buf();
        let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
            if let Ok(v2) = Vault::open(&root) {
                drop(connect_raw(&v2));
            }
        });

        let join = std::thread::spawn({
            let stop = Arc::clone(&stop);
            let last = Arc::clone(&last);
            let reason = Arc::clone(&reason);
            let wake = Arc::clone(&wake);
            move || {
                accept_loop(
                    &listener, &handler, &stop, &last, born, ttl, max, &reason, &wake,
                );
            }
        });
        Ok(Self {
            stop,
            wake,
            reason,
            join: Some(join),
            endpoint: ep,
            host: config.host,
            born,
            last,
            ttl,
            max,
        })
    }

    /// Blocks until the server exits (expiry, `lock` request, or `stop`)
    /// and cleans up the endpoint.
    #[must_use = "waiting consumes the server and cleans up the endpoint"]
    pub fn wait(mut self) -> StopReason {
        if let Some(join) = self.join.take() {
            drop(join.join());
        }
        self.endpoint.cleanup();
        self.reason
            .lock()
            .ok()
            .and_then(|r| *r)
            .unwrap_or(StopReason::Stopped)
    }

    /// Stops the server and cleans up; idempotent via [`StopHandle`].
    #[must_use = "stopping consumes the server and cleans up the endpoint"]
    pub fn stop(self) -> StopReason {
        self.stopper().stop();
        self.wait()
    }

    /// A handle for stopping the server from another thread.
    #[must_use]
    pub fn stopper(&self) -> StopHandle {
        StopHandle {
            stop: Arc::clone(&self.stop),
            wake: Arc::clone(&self.wake),
        }
    }

    /// Seconds until idle/lifetime expiry plus who is serving.
    #[must_use]
    pub fn status(&self) -> AgentStatus {
        AgentStatus {
            ttl_left: idle_left(&self.last, self.ttl),
            life_left: life_left(&self.born, self.max),
            host: self.host.as_str().to_owned(),
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        if self.join.is_some() {
            self.stopper().stop();
            if let Some(join) = self.join.take() {
                drop(join.join());
            }
            self.endpoint.cleanup();
        }
    }
}

struct Conn {
    handler: Arc<Handler>,
    stop: Arc<AtomicBool>,
    last: Arc<AtomicU64>,
    ttl: u64,
    born: Instant,
    max: u64,
    reason: Reason,
    wake: Arc<dyn Fn() + Send + Sync>,
}

#[allow(
    clippy::too_many_arguments,
    reason = "shared accept-loop state is clearer as parameters than a struct"
)]
fn accept_loop(
    listener: &Listener,
    handler: &Arc<Handler>,
    stop: &Arc<AtomicBool>,
    last: &Arc<AtomicU64>,
    born: Instant,
    ttl: u64,
    max: u64,
    reason: &Reason,
    wake: &Arc<dyn Fn() + Send + Sync>,
) {
    let own = own_uid();
    let _watchdog = std::thread::spawn({
        let stop = Arc::clone(stop);
        let last = Arc::clone(last);
        let reason = Arc::clone(reason);
        let wake = Arc::clone(wake);
        move || watchdog(&stop, &last, born, ttl, max, &reason, &wake)
    });
    while let Ok(stream) = listener.accept() {
        if stop.load(Ordering::Relaxed) || !same_user(&stream, own) {
            if stop.load(Ordering::Relaxed) {
                break;
            }
            continue;
        }
        std::thread::spawn({
            let conn = Conn {
                handler: Arc::clone(handler),
                stop: Arc::clone(stop),
                last: Arc::clone(last),
                ttl,
                born,
                max,
                reason: Arc::clone(reason),
                wake: Arc::clone(wake),
            };
            move || serve(stream, &conn)
        });
    }
    if reason.lock().map(|r| *r).ok().flatten().is_none()
        && let Ok(mut slot) = reason.lock()
    {
        *slot = Some(StopReason::Stopped);
    }
}

/// Serves one connection on its own thread; `lock` stops the agent and
/// wakes the blocked accept with a throwaway self-connect.
fn serve(stream: interprocess::local_socket::Stream, ctx: &Conn) {
    let end = ctx.handler.serve_connection(
        stream,
        &|| ctx.last.store(now_secs(), Ordering::Relaxed),
        &|| idle_left(&ctx.last, ctx.ttl),
        &|| life_left(&ctx.born, ctx.max),
    );
    if matches!(end, Ok(ConnectionEnd::LockRequested)) {
        if let Ok(mut slot) = ctx.reason.lock() {
            *slot = Some(StopReason::LockRequested);
        }
        ctx.stop.store(true, Ordering::Relaxed);
        (ctx.wake)();
    }
}

fn watchdog(
    stop: &AtomicBool,
    last: &AtomicU64,
    born: Instant,
    ttl: u64,
    max: u64,
    reason: &Reason,
    wake: &Arc<dyn Fn() + Send + Sync>,
) {
    while !stop.load(Ordering::Relaxed) {
        let idle = now_secs().saturating_sub(last.load(Ordering::Relaxed));
        let r = if idle >= ttl {
            Some(StopReason::Idle)
        } else if born.elapsed().as_secs() >= max {
            Some(StopReason::MaxLifetime)
        } else {
            None
        };
        if let Some(r) = r {
            if let Ok(mut slot) = reason.lock() {
                *slot = Some(r);
            }
            stop.store(true, Ordering::Relaxed);
            wake();
            return;
        }
        sleep_250();
    }
}

/// 250 ms poll for the watchdog — cheap and not in the request path.
#[allow(
    clippy::disallowed_methods,
    reason = "blocking sync daemon; the watchdog polls outside the request path"
)]
fn sleep_250() {
    std::thread::sleep(Duration::from_millis(250));
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
fn own_uid() -> u32 {
    rustix::process::getuid().as_raw()
}

#[cfg(windows)]
const fn own_uid() -> u32 {
    0
}

#[cfg(unix)]
fn same_user(stream: &interprocess::local_socket::Stream, own: u32) -> bool {
    stream.peer_creds().ok().and_then(|c| c.euid()) == Some(own)
}

#[cfg(windows)]
const fn same_user(_s: &interprocess::local_socket::Stream, _o: u32) -> bool {
    true
}

#[cfg(windows)]
fn owner_only_dacl() -> Result<interprocess::os::windows::security_descriptor::SecurityDescriptor> {
    use interprocess::os::windows::security_descriptor::SecurityDescriptor;
    let sddl = widestring::U16CString::from_str("D:P(A;;GA;;;OW)")
        .map_err(|e| Error::Unavailable(format!("bad SDDL: {e}")))?;
    SecurityDescriptor::deserialize(&sddl)
        .map_err(|e| Error::Unavailable(format!("bad security descriptor: {e}")))
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::missing_assert_message,
    reason = "tests: panicking asserts"
)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn vault() -> (tempfile::TempDir, Vault) {
        let dir = tempfile::tempdir().expect("tmpdir");
        let root = dir.path().join("vault");
        Vault::create(
            &root,
            &SecretString::from("a long enough test passphrase".to_owned()),
        )
        .expect("init");
        let v = Vault::open(&root).expect("open");
        (dir, v)
    }

    fn identity() -> SecretString {
        SecretString::from(crate::LocalKey::generate().to_identity_text())
    }

    fn cfg(ttl: u64, max: u64, host: Host) -> ServerConfig {
        ServerConfig {
            ttl: Duration::from_secs(ttl),
            max: Duration::from_secs(max),
            host,
        }
    }

    #[test]
    fn server_reports_host_and_status() {
        let (_t, v) = vault();
        let server = Server::start(&v, identity(), cfg(30, 300, Host::Desktop))
            .expect("server should start");
        let st = server.status();
        assert_eq!(st.host, "desktop");
        assert!(st.ttl_left <= 30);
        let remote = crate::agent::status(&v).expect("status").expect("running");
        assert_eq!(remote.host, "desktop");
        assert_eq!(server.stop(), StopReason::Stopped);
    }

    #[test]
    fn server_lock_request_exits() {
        let (_t, v) = vault();
        let server =
            Server::start(&v, identity(), cfg(30, 300, Host::Cli)).expect("server should start");
        assert!(crate::agent::lock(&v).expect("lock"));
        assert_eq!(server.wait(), StopReason::LockRequested);
    }

    #[test]
    fn server_idles_out() {
        let (_t, v) = vault();
        let server =
            Server::start(&v, identity(), cfg(1, 300, Host::Cli)).expect("server should start");
        assert_eq!(server.wait(), StopReason::Idle);
    }

    #[test]
    fn second_start_refuses() {
        let (_t, v) = vault();
        let server =
            Server::start(&v, identity(), cfg(30, 300, Host::Cli)).expect("server should start");
        match Server::start(&v, identity(), cfg(30, 300, Host::Cli)) {
            Err(Error::Unavailable(_)) => {}
            _ => panic!("second agent must refuse"),
        }
        let _r = server.stop();
    }
}
