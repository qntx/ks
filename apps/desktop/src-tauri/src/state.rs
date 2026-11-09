//! Application state: the open vault, the unlocked `Session`, the file
//! watcher and lock timers. Locking drops the Session (key zeroized).

use std::sync::Mutex;
use std::time::Instant;

use crate::watch::Watcher;

/// Everything related to a currently-open vault.
pub(crate) struct Open {
    /// The opened vault.
    pub vault: ks::Vault,
    /// The unlocked session; `None` while locked.
    pub session: Option<ks::Session>,
    /// Filesystem watcher feeding `EntriesChanged`/`key_changed`.
    pub watcher: Option<Watcher>,
    /// Last unwrap/wrap or command activity.
    pub last_activity: Instant,
    /// When the session was unlocked.
    pub unlocked_at: Option<Instant>,
}

/// Shared app state behind one mutex; commands borrow it briefly.
pub(crate) struct AppState {
    /// Inner state.
    pub inner: Mutex<Inner>,
}

/// The vault slot; `None` before a vault is opened.
pub(crate) struct Inner {
    /// The open vault, if any.
    pub open: Option<Open>,
}

impl AppState {
    /// Fresh state (no vault, locked).
    pub(crate) const fn new() -> Self {
        Self {
            inner: Mutex::new(Inner { open: None }),
        }
    }

    /// Marks user activity (heartbeat/command) — resets the idle timer.
    pub(crate) fn touch(&self) {
        if let Ok(mut i) = self.inner.lock()
            && let Some(o) = i.open.as_mut()
        {
            o.last_activity = Instant::now();
        }
    }

    /// Locks: drops the session and returns the reason to emit.
    pub(crate) fn lock(&self, reason: crate::lock::Reason) -> bool {
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
        crate::lock::on_locked(reason);
        true
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
        Ok(r)
    }
}
