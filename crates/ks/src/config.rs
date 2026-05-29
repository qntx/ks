//! Runtime configuration: filesystem paths.
//!
//! Resolution order (highest priority first):
//! 1. Environment variables (`KS_DIR`, `KS_STORE_DIR`, `KS_IDENTITY`).
//! 2. Platform defaults from the `directories` crate
//!    (`$XDG_DATA_HOME/ks` on Linux, the equivalent elsewhere).
//!
//! There is deliberately no config file: everything tunable is an environment
//! variable, in the spirit of `pass`. This keeps the store fully described by
//! its directory and the surrounding environment.

use std::path::PathBuf;

use directories::ProjectDirs;

use crate::error::{Error, Result};

const APP: &str = "ks";
const IDENTITY_FILE: &str = "identity.age";
const STORE_DIR: &str = "store";
const RECIPIENTS_FILE: &str = ".age-recipients";

/// Base data directory holding `identity.age` and `store/`.
pub const ENV_DIR: &str = "KS_DIR";
/// Explicit store directory (overrides `KS_DIR/store`).
pub const ENV_STORE_DIR: &str = "KS_STORE_DIR";
/// Explicit identity file path (overrides `KS_DIR/identity.age`).
pub const ENV_IDENTITY: &str = "KS_IDENTITY";

/// Resolved filesystem paths for a store and its identity.
#[derive(Debug, Clone)]
pub struct Config {
    /// Identity file path (`identity.age`).
    pub identity_path: PathBuf,
    /// Store root directory (contains `.age-recipients` and encrypted secrets).
    pub store_dir: PathBuf,
}

impl Config {
    /// Resolves configuration from environment variables and platform defaults.
    ///
    /// # Errors
    /// Returns [`Error::NoUserDir`] if no `KS_DIR` is set and platform data
    /// directories cannot be determined.
    pub fn load() -> Result<Self> {
        let data_dir = match env_path(ENV_DIR) {
            Some(dir) => dir,
            None => ProjectDirs::from("", "", APP)
                .ok_or(Error::NoUserDir)?
                .data_dir()
                .to_path_buf(),
        };
        Ok(Self {
            identity_path: env_path(ENV_IDENTITY).unwrap_or_else(|| data_dir.join(IDENTITY_FILE)),
            store_dir: env_path(ENV_STORE_DIR).unwrap_or_else(|| data_dir.join(STORE_DIR)),
        })
    }

    /// Path to the recipients file inside the store.
    #[must_use]
    pub fn recipients_path(&self) -> PathBuf {
        self.store_dir.join(RECIPIENTS_FILE)
    }
}

fn env_path(name: &str) -> Option<PathBuf> {
    match std::env::var(name) {
        Ok(raw) if !raw.is_empty() => Some(PathBuf::from(raw)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recipients_path_is_under_store() {
        let cfg = Config {
            identity_path: PathBuf::from("/tmp/ks/identity.age"),
            store_dir: PathBuf::from("/tmp/ks/store"),
        };
        assert_eq!(
            cfg.recipients_path(),
            PathBuf::from("/tmp/ks/store/.age-recipients")
        );
    }
}
