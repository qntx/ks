//! Application state: the open vault, the unlocked `Session`, the file
//! watcher and lock timers. Locking drops the Session (key zeroized).

use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::watch::Watcher;

/// Everything related to a currently-open vault.
pub(crate) struct Open {
    /// The opened vault.
    pub vault: ks::Vault,
    /// The unlocked session; `None` while locked.
    pub session: Option<ks::Session>,
    /// Filesystem watcher feeding `EntriesChanged`/`key_changed`.
    pub watcher: Option<Watcher>,
    /// Auto-lock watchdog stop channel; `None` while locked.
    pub watchdog: Option<Sender<crate::lock::Wake>>,
    /// Hosted agent endpoint while unlocked; terminal traffic counts as
    /// activity through `ServerHandle::touch`.
    pub agent: Option<ks::agent::ServerHandle>,
    /// Last unwrap/wrap or command activity.
    pub last_activity: Instant,
    /// When the session was unlocked.
    pub unlocked_at: Option<Instant>,
}

impl Open {
    /// Stops the auto-lock watchdog, if running.
    fn stop_watchdog(&mut self) {
        if let Some(tx) = self.watchdog.take() {
            drop(tx.send(crate::lock::Wake));
        }
    }

    /// Stops the hosted agent, if running.
    fn stop_agent(&mut self) {
        if let Some(h) = self.agent.take() {
            h.stop();
        }
    }

    /// (Re)starts the auto-lock watchdog for this session.
    pub(crate) fn start_watchdog(&mut self, state: Arc<AppState>, idle: Duration, max: Duration) {
        self.stop_watchdog();
        self.watchdog = Some(crate::lock::start(state, idle, max));
    }
}

impl Drop for Open {
    fn drop(&mut self) {
        self.stop_watchdog();
        self.stop_agent();
    }
}

/// Shared app state behind one mutex; commands borrow it briefly.
pub(crate) struct AppState {
    /// Single-flight sync slot — never held across git I/O.
    pub(crate) sync: Mutex<crate::sync::SyncSlot>,
    /// Serializes every app-initiated git process. Write commands hold
    /// it across file write + autocommit so a running sync can never
    /// stage a half-written change; a write during a sync simply waits.
    ///
    /// Lock order: `git` is outermost — `inner`/`sync` may be acquired
    /// while it is held, but `inner`/`sync` are never held while
    /// waiting on `git`.
    pub(crate) git: Mutex<()>,
    /// Inner state.
    pub inner: Mutex<Inner>,
}

/// The vault slot; `None` before a vault is opened.
pub(crate) struct Inner {
    /// The open vault, if any.
    pub open: Option<Open>,
    /// Why the last lock happened (the event carries it too).
    pub last_lock_reason: Option<crate::lock::Reason>,
}

impl AppState {
    /// Fresh state (no vault, locked).
    pub(crate) const fn new() -> Self {
        Self {
            inner: Mutex::new(Inner {
                open: None,
                last_lock_reason: None,
            }),
            sync: Mutex::new(crate::sync::SyncSlot {
                running: false,
                state: crate::sync::SyncState::Idle,
                write_due: None,
                debounce_pending: false,
            }),
            git: Mutex::new(()),
        }
    }

    /// Marks user activity (heartbeat/command) — resets the idle timer.
    pub(crate) fn touch(&self) {
        if let Ok(mut i) = self.inner.lock()
            && let Some(o) = i.open.as_mut()
        {
            o.last_activity = Instant::now();
            if let Some(a) = &o.agent {
                a.touch();
            }
        }
    }

    /// Locks: drops the session and returns the reason to emit.
    pub(crate) fn lock(self: &Arc<Self>, reason: crate::lock::Reason) -> bool {
        let locked = {
            let mut i = match self.inner.lock() {
                Ok(i) => i,
                Err(_) => return false,
            };
            let Some(o) = i.open.as_mut() else {
                return false;
            };
            if o.session.take().is_none() {
                return false;
            }
            o.unlocked_at = None;
            o.stop_watchdog();
            o.stop_agent();
            i.last_lock_reason = Some(reason);
            true
        };
        crate::lock::on_locked(reason);
        if reason != crate::lock::Reason::Quit {
            crate::sync::after_lock(self);
        }
        locked
    }

    /// Runs `f` with the unlocked session (and bumps the activity
    /// clock); `locked` when there is none.
    pub(crate) fn with_session<T>(
        &self,
        f: impl FnOnce(&ks::Session) -> Result<T, ks::Error>,
    ) -> Result<T, ks::Error> {
        let mut i = self
            .inner
            .lock()
            .map_err(|_| ks::Error::Internal("state lock poisoned"))?;
        let o = i.open.as_mut().ok_or(ks::Error::Locked)?;
        let s = o.session.as_ref().ok_or(ks::Error::Locked)?;
        let r = f(s)?;
        o.last_activity = Instant::now();
        if let Some(a) = &o.agent {
            a.touch();
        }
        Ok(r)
    }
}
