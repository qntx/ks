//! Non-secret settings persisted as JSON in the app config dir.

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
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            auto_lock_minutes: 15,
            max_session_hours: 8,
            clipboard_seconds: 45,
            lock_on_sleep: true,
            language: "system".to_owned(),
        }
    }
}

/// Path of the settings file under the app config dir.
pub(crate) fn path() -> std::path::PathBuf {
    directories::BaseDirs::new().map_or_else(
        || std::path::PathBuf::from("ks-settings.json"),
        |d| d.config_dir().join("ks").join("settings.json"),
    )
}

/// Loads settings; missing/corrupt file yields defaults.
pub(crate) fn load() -> Settings {
    std::fs::read(path())
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

/// Saves settings atomically with 0600 permissions.
pub(crate) fn save(s: &Settings) -> Result<(), std::io::Error> {
    let p = path();
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = p.with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_vec(s)?)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
    }
    std::fs::rename(&tmp, &p)
}
