//! Filesystem watch on the vault root: debounced `EntriesChanged` events
//! and a `key_changed` lock when `.identity.age` is replaced.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use notify::RecursiveMode;
use notify_debouncer_mini::{DebounceEventResult, Debouncer, new_debouncer};
use sha2::Digest as _;
use tauri::Emitter as _;

use crate::lock::{Reason, lock_now};
use crate::state::AppState;

/// Emitted when vault files change on disk.
#[derive(Debug, Clone, serde::Serialize, specta::Type, tauri_specta::Event)]
pub(crate) struct EntriesChanged;

/// Live watcher; dropping it stops watching.
pub(crate) struct Watcher {
    _d: Debouncer<notify::RecommendedWatcher>,
}

fn sha(path: &Path) -> Option<Vec<u8>> {
    std::fs::read(path)
        .ok()
        .map(|b| sha2::Sha256::digest(&b).to_vec())
}

fn relevant(path: &Path) -> bool {
    !path
        .components()
        .any(|c| c.as_os_str() == ".git" || c.as_os_str() == ".lock")
}

fn on_events<RT: tauri::Runtime>(
    res: DebounceEventResult,
    app: &tauri::AppHandle<RT>,
    state: &Arc<AppState>,
    identity: &Path,
    identity_hash: &[u8],
) {
    let Ok(events) = res else {
        return;
    };
    if events.iter().any(|e| relevant(&e.path)) {
        drop(app.emit("entries-changed", EntriesChanged));
    }
    let identity_changed = events.iter().any(|e| e.path == identity)
        && sha(identity).as_deref() != Some(identity_hash);
    if identity_changed {
        lock_now(state, Reason::KeyChanged);
    }
}

impl Watcher {
    /// Starts watching `root`; `identity_hash` is `.identity.age`'s
    /// SHA-256 at unlock — a changed hash locks with `key_changed`.
    pub(crate) fn start<RT: tauri::Runtime>(
        app: tauri::AppHandle<RT>,
        state: Arc<AppState>,
        root: PathBuf,
        identity_hash: Vec<u8>,
    ) -> Option<Self> {
        let root = root.canonicalize().unwrap_or(root);
        let identity = root.join(".identity.age");
        let mut d = new_debouncer(Duration::from_millis(250), move |res| {
            on_events(res, &app, &state, &identity, &identity_hash);
        })
        .ok()?;
        d.watcher().watch(&root, RecursiveMode::Recursive).ok()?;
        Some(Self { _d: d })
    }
}
