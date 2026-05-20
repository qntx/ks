//! Runtime configuration: paths and tunables.
//!
//! Resolution order (highest priority wins):
//! 1. Explicit overrides on [`Config`] (programmatic).
//! 2. Environment variables (`KS_*`).
//! 3. `$XDG_CONFIG_HOME/ks/config.toml` (or platform equivalent).
//! 4. Platform defaults from the `directories` crate.

use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

const APP: &str = "ks";
const CONFIG_FILE: &str = "config.toml";
const IDENTITY_FILE: &str = "identity.age";
const STORE_DIR: &str = "store";
const RECIPIENTS_FILE: &str = ".recipients";

/// Environment variable: full path to a `config.toml` file.
pub const ENV_CONFIG: &str = "KS_CONFIG";
/// Environment variable: directory containing `identity.age` and `store/`.
pub const ENV_DATA_DIR: &str = "KS_DATA_DIR";
/// Environment variable: explicit store directory (overrides `KS_DATA_DIR/store`).
pub const ENV_STORE_DIR: &str = "KS_STORE_DIR";
/// Environment variable: explicit identity file path.
pub const ENV_IDENTITY: &str = "KS_IDENTITY";

/// Tunables that may be persisted in `config.toml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Tunables {
    /// Session cache lifetime in seconds (default: 900 = 15 minutes).
    pub session_ttl_secs: u64,
    /// Clipboard auto-clear delay in seconds (default: 45).
    pub clipboard_clear_secs: u64,
}

impl Default for Tunables {
    fn default() -> Self {
        Self {
            session_ttl_secs: 900,
            clipboard_clear_secs: 45,
        }
    }
}

/// Resolved runtime configuration.
#[derive(Debug, Clone)]
pub struct Config {
    /// Identity file path (`identity.age`).
    pub identity_path: PathBuf,
    /// Store root directory (contains `.recipients` and encrypted secrets).
    pub store_dir: PathBuf,
    /// Path to the config file (may not exist on disk yet).
    pub config_path: PathBuf,
    /// Tunable values.
    pub tunables: Tunables,
}

impl Config {
    /// Resolves configuration from env and platform defaults.
    ///
    /// # Errors
    /// Returns [`Error::NoUserDir`] if platform directories cannot be determined.
    /// Returns [`Error::Toml`] / [`Error::Io`] if `config.toml` exists but is malformed.
    pub fn load() -> Result<Self> {
        let dirs = ProjectDirs::from("", "", APP).ok_or(Error::NoUserDir)?;
        let default_config = dirs.config_dir().join(CONFIG_FILE);
        let default_data = dirs.data_dir().to_path_buf();

        let config_path = env_path(ENV_CONFIG).unwrap_or(default_config);
        let data_dir = env_path(ENV_DATA_DIR).unwrap_or(default_data);

        let identity_path = env_path(ENV_IDENTITY).unwrap_or_else(|| data_dir.join(IDENTITY_FILE));
        let store_dir = env_path(ENV_STORE_DIR).unwrap_or_else(|| data_dir.join(STORE_DIR));

        let tunables = if config_path.exists() {
            let text = std::fs::read_to_string(&config_path)?;
            toml::from_str(&text)?
        } else {
            Tunables::default()
        };

        Ok(Self {
            identity_path,
            store_dir,
            config_path,
            tunables,
        })
    }

    /// Path to the recipients file inside the store.
    #[must_use]
    pub fn recipients_path(&self) -> PathBuf {
        self.store_dir.join(RECIPIENTS_FILE)
    }

    /// Persists current [`Tunables`] to disk at `config_path`.
    ///
    /// Creates the parent directory if needed.
    ///
    /// # Errors
    /// Returns [`Error::Io`] / [`Error::Toml`] on serialisation or write failures.
    pub fn save_tunables(&self) -> Result<()> {
        if let Some(parent) = self.config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = toml::to_string_pretty(&self.tunables)?;
        std::fs::write(&self.config_path, text)?;
        Ok(())
    }
}

fn env_path(name: &str) -> Option<PathBuf> {
    let raw = std::env::var(name).ok()?;
    if raw.is_empty() {
        None
    } else {
        Some(PathBuf::from(raw))
    }
}

/// Returns a short stable identifier for a store directory, used as part of the
/// OS keyring entry name. Collisions are acceptable because the keyring entry
/// also embeds the absolute path in plaintext metadata.
#[must_use]
pub fn store_id(store_dir: &Path) -> String {
    use std::hash::{Hash as _, Hasher as _};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    store_dir.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tunables_default_values() {
        let t = Tunables::default();
        assert_eq!(t.session_ttl_secs, 900);
        assert_eq!(t.clipboard_clear_secs, 45);
    }

    #[test]
    fn store_id_is_stable() {
        let a = store_id(Path::new("/tmp/ks/store"));
        let b = store_id(Path::new("/tmp/ks/store"));
        assert_eq!(a, b);
        assert_eq!(a.len(), 16);
    }
}
