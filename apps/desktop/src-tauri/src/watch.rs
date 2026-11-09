//! Filesystem watch on the vault root: debounced `EntriesChanged` events
//! and a `key_changed` lock when `.identity.age` is replaced.
#![allow(
    clippy::excessive_nesting,
    reason = "debouncer closure: flat filter chain is less readable than one nested level"
)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use notify::RecursiveMode;
use notify_debouncer_mini::{DebounceEventResult, Debouncer, new_debouncer};
use sha2::Digest as _;
use tauri::{AppHandle, Emitter as _};

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

impl Watcher {
    /// Starts watching `root`; `identity_hash` is `.identity.age`'s
    /// SHA-256 at unlock — a changed hash locks with `key_changed`.
    pub(crate) fn start(
        app: AppHandle,
        state: Arc<AppState>,
        root: PathBuf,
        identity_hash: Vec<u8>,
    ) -> Option<Self> {
        let identity = root.join(".identity.age");
        let mut d = new_debouncer(Duration::from_millis(250), {
            let app = app;
            move |res: DebounceEventResult| {
                let Ok(events) = res else { return };
                if events.iter().any(|e| relevant(&e.path)) {
                    drop(app.emit("entries-changed", EntriesChanged));
                }
                if events.iter().any(|e| e.path == identity)
                    && sha(&identity).as_deref() != Some(identity_hash.as_slice())
                {
                    lock_now(&state, Reason::KeyChanged);
                }
            }
        })
        .ok()?;
        d.watcher().watch(&root, RecursiveMode::Recursive).ok()?;
        Some(Self { _d: d })
    }
}
