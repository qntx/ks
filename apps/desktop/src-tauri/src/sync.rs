//! Vault sync (D3): `GitStatus` reporting, the single-flight sync runner
//! behind `sync-state` events, and the auto-sync triggers — on unlock,
//! on lock, and a debounced write follow-up. Git runs need no session;
//! the state mutexes are never held across git I/O.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::Emitter as _;

use crate::error::AppError;

type R<T> = Result<T, AppError>;
use crate::state::AppState;

/// Repo state for the sidebar pill; `None`-ish mirrors of the library.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GitStatus {
    /// Current branch (detached HEAD → `null`).
    pub branch: Option<String>,
    /// Configured upstream, if any.
    pub upstream: Option<String>,
    /// Commits ahead of the upstream.
    pub ahead: u32,
    /// Commits behind the upstream.
    pub behind: u32,
    /// Uncommitted changes in the syncable paths.
    pub changes: u32,
}

impl From<ks::git::GitStatus> for GitStatus {
    fn from(s: ks::git::GitStatus) -> Self {
        Self {
            branch: s.branch,
            upstream: s.upstream,
            ahead: s.ahead,
            behind: s.behind,
            changes: s.changes,
        }
    }
}

/// Which side wins a content conflict (`"local"` or `"remote"`).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Keep {
    /// Keep local entries.
    Local,
    /// Keep remote entries.
    Remote,
}

impl From<Keep> for ks::git::Keep {
    fn from(k: Keep) -> Self {
        match k {
            Keep::Local => Self::Local,
            Keep::Remote => Self::Remote,
        }
    }
}

/// Conflict class (`"content"` | `"delete"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ConflictKind {
    /// Same entry edited on both sides — retryable with `keep`.
    Content,
    /// One side deleted what the other edited — manual steps only.
    Delete,
}

impl From<ks::ConflictKind> for ConflictKind {
    fn from(k: ks::ConflictKind) -> Self {
        match k {
            ks::ConflictKind::Content => Self::Content,
            ks::ConflictKind::Delete => Self::Delete,
        }
    }
}

/// Outcome of one sync run; a conflict is a result, not an error.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(crate) enum SyncOutcome {
    /// Pull/commit/push completed.
    Synced {
        /// Uncommitted changes were committed first.
        committed: bool,
        /// A pull ran.
        pulled: bool,
        /// A push ran.
        pushed: bool,
        /// Non-fatal observations.
        warnings: Vec<String>,
    },
    /// Merge conflicts the UI offers to resolve.
    Conflict {
        /// Conflicting entry names.
        entries: Vec<String>,
        /// `content` (retryable) or `delete` (manual only).
        kind: ConflictKind,
    },
}

/// Live sync state for the pill, emitted as the `sync-state` event.
#[derive(Debug, Clone, Serialize, Type, tauri_specta::Event)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(crate) enum SyncState {
    /// Nothing happening.
    Idle,
    /// A run is in flight.
    Running,
    /// Last run succeeded; `at_secs` is the unix timestamp.
    Synced {
        /// Unix seconds when it finished.
        at_secs: u32,
    },
    /// Last run hit conflicts.
    Conflict {
        /// Conflicting entry names.
        entries: Vec<String>,
        /// `content` or `delete`.
        kind: ConflictKind,
    },
    /// Last run failed (network, auth, …).
    Failed {
        /// Human-readable error.
        message: String,
    },
}

/// Single-flight slot on [`AppState`]; the mutex only guards these fields.
pub(crate) struct SyncSlot {
    /// A run is in flight.
    pub running: bool,
    /// The state the pill shows.
    pub state: SyncState,
    /// Debounced write-triggered deadline.
    pub write_due: Option<Instant>,
    /// A debounce thread is waiting on `write_due`.
    pub debounce_pending: bool,
}

/// Emits the `sync-state` event — a no-op until `lock::init` runs.
fn emit(state: &SyncState) {
    if let Some(app) = &*crate::lock::APP
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
    {
        drop(app.emit("sync-state", state.clone()));
    }
}

/// Runs one git sync for `root` under the single-flight guard. Refuses
/// with `unavailable` while a run is in flight. State mutexes are never
/// held across git I/O.
pub(crate) fn run_sync(
    state: &Arc<AppState>,
    root: &Path,
    keep: Option<ks::git::Keep>,
) -> R<SyncOutcome> {
    {
        let mut s = state
            .sync
            .lock()
            .map_err(|_| ks::Error::Internal("state lock poisoned"))?;
        if s.running {
            return Err(ks::Error::Unavailable("a sync is already running".into()).into());
        }
        s.running = true;
        s.state = SyncState::Running;
    }
    emit(&SyncState::Running);
    let result = {
        let _guard = state
            .git
            .lock()
            .map_err(|_| ks::Error::Internal("state lock poisoned"))?;
        ks::git::Git::new(root, false).sync(keep)
    };
    let (outcome, final_state, err) = match result {
        Ok(r) => (
            Some(SyncOutcome::Synced {
                committed: r.committed,
                pulled: r.pulled,
                pushed: r.pushed,
                warnings: r.warnings,
            }),
            SyncState::Synced {
                at_secs: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| u32::try_from(d.as_secs()).unwrap_or(u32::MAX)),
            },
            None,
        ),
        Err(ks::Error::Conflict { entries, kind }) => (
            Some(SyncOutcome::Conflict {
                entries: entries.clone(),
                kind: kind.into(),
            }),
            SyncState::Conflict {
                entries,
                kind: kind.into(),
            },
            None,
        ),
        Err(e) => (
            None,
            SyncState::Failed {
                message: e.to_string(),
            },
            Some(AppError::from(e)),
        ),
    };
    {
        let mut s = state
            .sync
            .lock()
            .map_err(|_| ks::Error::Internal("state lock poisoned"))?;
        s.state = final_state.clone();
        s.running = false;
    }
    emit(&final_state);
    if let Some(e) = err {
        return Err(e);
    }
    outcome.ok_or_else(|| ks::Error::Internal("sync outcome missing").into())
}

/// Status of the open vault's repo; `None` when it is not a repo.
#[tauri::command(async)]
#[specta::specta]
#[expect(
    clippy::needless_pass_by_value,
    reason = "tauri commands receive `State` by value"
)]
pub(crate) fn git_status(state: tauri::State<'_, Arc<AppState>>) -> R<Option<GitStatus>> {
    let root = {
        let i = state
            .inner
            .lock()
            .map_err(|_| ks::Error::Internal("state lock poisoned"))?;
        i.open
            .as_ref()
            .map(|o| o.vault.root().to_path_buf())
            .ok_or(ks::Error::Locked)?
    };
    let g = ks::git::Git::new(&root, false);
    if !g.is_repo() {
        return Ok(None);
    }
    Ok(Some(g.status()?.into()))
}

/// Runs a sync now on a blocking thread; `keep` resolves a conflict.
#[tauri::command]
#[specta::specta]
pub(crate) async fn sync_now(
    state: tauri::State<'_, Arc<AppState>>,
    keep: Option<Keep>,
) -> Result<SyncOutcome, AppError> {
    let root = {
        let i = state
            .inner
            .lock()
            .map_err(|_| ks::Error::Internal("state lock poisoned"))?;
        i.open
            .as_ref()
            .map(|o| o.vault.root().to_path_buf())
            .ok_or(ks::Error::Locked)?
    };
    let arc = Arc::clone(state.inner());
    let keep = keep.map(ks::git::Keep::from);
    tauri::async_runtime::spawn_blocking(move || run_sync(&arc, &root, keep))
        .await
        .map_err(|_| ks::Error::Internal("sync task panicked"))?
}

/// Current sync state for the pill.
#[tauri::command(async)]
#[specta::specta]
#[expect(
    clippy::needless_pass_by_value,
    reason = "tauri commands receive `State` by value"
)]
pub(crate) fn sync_state(state: tauri::State<'_, Arc<AppState>>) -> SyncState {
    state
        .sync
        .lock()
        .map_or(SyncState::Idle, |s| s.state.clone())
}

/// True when the open vault is a git repo; used by every auto trigger.
fn is_git_vault(state: &Arc<AppState>) -> Option<PathBuf> {
    let root = state
        .inner
        .lock()
        .ok()?
        .open
        .as_ref()
        .map(|o| o.vault.root().to_path_buf())?;
    ks::git::Git::new(&root, false).is_repo().then_some(root)
}

/// Spawns a background sync when `root` qualifies; `unavailable` while
/// another sync runs is ignored silently.
fn spawn_auto(root: PathBuf, state: Arc<AppState>) {
    std::thread::Builder::new()
        .name("sync-auto".to_owned())
        .spawn(move || drop(run_sync(&state, &root, None)))
        .map_or_else(drop, drop);
}

/// Trigger after a successful unlock (`sync_on_unlock`).
pub(crate) fn after_unlock(state: &Arc<AppState>) {
    if !crate::settings::load().sync_on_unlock {
        return;
    }
    if let Some(root) = is_git_vault(state) {
        spawn_auto(root, Arc::clone(state));
    }
}

/// Trigger after a successful lock (`sync_on_lock`).
pub(crate) fn after_lock(state: &Arc<AppState>) {
    if !crate::settings::load().sync_on_lock {
        return;
    }
    if let Some(root) = is_git_vault(state) {
        spawn_auto(root, Arc::clone(state));
    }
}

/// Marks a write for the 30 s debounce (`sync_after_write`); spawns the
/// debounce thread only when none is pending.
pub(crate) fn after_write(state: &Arc<AppState>) {
    if !crate::settings::load().sync_after_write {
        return;
    }
    if is_git_vault(state).is_none() {
        return;
    }
    {
        let mut s = state
            .sync
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        s.write_due = Some(Instant::now() + Duration::from_secs(30));
        if s.debounce_pending {
            return;
        }
        s.debounce_pending = true;
    }
    let st2 = Arc::clone(state);
    drop(
        std::thread::Builder::new()
            .name("sync-write-due".to_owned())
            .spawn(move || debounce(&st2)),
    );
}

/// Debounce worker: waits out `write_due` (new writes push it out),
/// waits for an in-flight run, then syncs and frees the slot.
#[allow(
    clippy::disallowed_methods,
    reason = "the debounce timer legitimately sleeps"
)]
fn debounce(st: &Arc<AppState>) {
    loop {
        let due = st.sync.lock().ok().and_then(|s| s.write_due);
        let Some(t) = due else {
            break;
        };
        let left = t.saturating_duration_since(Instant::now());
        if left.is_zero() {
            break;
        }
        std::thread::sleep(left.min(Duration::from_secs(5)));
    }
    while st.sync.lock().is_ok_and(|s| s.running) {
        std::thread::sleep(Duration::from_secs(1));
    }
    {
        let mut s = st
            .sync
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        s.write_due.take();
        s.debounce_pending = false;
    }
    if let Some(root) = is_git_vault(st) {
        drop(run_sync(st, &root, None));
    }
}

/// Quit path: locks with `Reason::Quit` (which deliberately skips the
/// lock trigger), then runs one sync — or waits for an in-flight one —
/// up to a 5 s deadline before the caller exits.
#[allow(
    clippy::disallowed_methods,
    reason = "bounded blocking wait on the quit path"
)]
pub(crate) fn quit(state: &Arc<AppState>) {
    let _ = state.lock(crate::lock::Reason::Quit);
    if !crate::settings::load().sync_on_lock {
        return;
    }
    let Some(root) = is_git_vault(state) else {
        return;
    };
    let deadline = Instant::now() + Duration::from_secs(5);
    if state.sync.lock().is_ok_and(|s| s.running) {
        // A sync started earlier is in flight — just wait for it.
        while Instant::now() < deadline && state.sync.lock().is_ok_and(|s| s.running) {
            std::thread::sleep(Duration::from_millis(50));
        }
        return;
    }
    let (tx, rx) = std::sync::mpsc::channel();
    let st2 = Arc::clone(state);
    drop(
        std::thread::Builder::new()
            .name("sync-quit".to_owned())
            .spawn(move || drop(tx.send(run_sync(&st2, &root, None)))),
    );
    drop(rx.recv_timeout(deadline.saturating_duration_since(Instant::now())));
}
