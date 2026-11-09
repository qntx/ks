//! Non-secret settings persisted as JSON in the app config dir.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use specta::Type;

/// User-facing preferences; defaults match `ks unlock`.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct Settings {
    /// Idle minutes before auto-lock (0 = never).
    pub auto_lock_minutes: u32,
    /// Hard session cap in hours (0 = none).
    pub max_session_hours: u32,
    /// Seconds before a copied secret is cleared.
    pub clipboard_seconds: u32,
    /// Lock on system sleep/screen lock.
    pub lock_on_sleep: bool,
    /// UI language: "system", "en" or "zh".
    pub language: String,
    /// Sync right after the vault unlocks.
    pub sync_on_unlock: bool,
    /// Sync right after the vault locks.
    pub sync_on_lock: bool,
    /// Debounced sync after writes.
    pub sync_after_write: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            auto_lock_minutes: 15,
            max_session_hours: 8,
            clipboard_seconds: 45,
            lock_on_sleep: true,
            language: "system".to_owned(),
            sync_on_unlock: true,
            sync_on_lock: true,
            sync_after_write: false,
        }
    }
}

/// The app's config dir. Under `cfg(test)` it is a per-process temp dir
/// so tests never touch the real user config.
fn dir() -> PathBuf {
    if cfg!(test) {
        return std::env::temp_dir().join(format!("ks-desktop-{}", std::process::id()));
    }
    directories::BaseDirs::new().map_or_else(|| PathBuf::from("ks"), |d| d.config_dir().join("ks"))
}

/// Path of the settings file under the app config dir.
pub(crate) fn path() -> PathBuf {
    dir().join("settings.json")
}

/// Path of the app state file (last vault) under the app config dir.
fn state_path() -> PathBuf {
    dir().join("state.json")
}

/// Atomically writes `bytes` to `p` with 0600 permissions.
fn write_atomic(p: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d)?;
    }
    let tmp = p.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
    }
    std::fs::rename(&tmp, p)
}

/// Loads settings; missing/corrupt file yields defaults.
pub(crate) fn load() -> Settings {
    std::fs::read(path())
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

/// Saves settings atomically with 0600 permissions.
pub(crate) fn save(s: &Settings) -> std::io::Result<()> {
    write_atomic(&path(), &serde_json::to_vec(s)?)
}

/// The last vault successfully opened, created or cloned.
pub(crate) fn last_vault() -> Option<PathBuf> {
    let bytes = std::fs::read(state_path()).ok()?;
    let state: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    state.get("lastVault")?.as_str().map(PathBuf::from)
}

/// Remembers `path` as the last-used vault; persistence is
/// best-effort, so write failures are ignored.
pub(crate) fn remember_vault(path: &Path) {
    let state = serde_json::json!({ "lastVault": path.display().to_string() });
    if let Ok(bytes) = serde_json::to_vec(&state) {
        drop(write_atomic(&state_path(), &bytes));
    }
}
