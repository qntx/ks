use std::path::PathBuf;

use directories::ProjectDirs;

use crate::error::{Error, Result};

const APP: &str = "ks";
const VAULT_FILE: &str = "vault.age";
/// Environment variable that overrides the default vault path.
pub const ENV_VAULT_PATH: &str = "KS_VAULT_PATH";

/// Runtime configuration resolved from environment or platform defaults.
#[derive(Debug, Clone)]
pub struct Config {
    /// Absolute path to the encrypted vault file.
    pub vault_path: PathBuf,
}

impl Config {
    /// Resolves configuration from the environment.
    ///
    /// If `KS_VAULT_PATH` is set, that path is used directly.
    /// Otherwise the platform-appropriate data directory is used:
    /// - **Linux**: `$XDG_DATA_HOME/ks/vault.age`
    /// - **Windows**: `%APPDATA%\ks\data\vault.age`
    /// - **macOS**: `~/Library/Application Support/ks/vault.age`
    ///
    /// # Errors
    /// Returns [`Error::NoHomeDir`] when no home directory can be determined.
    pub fn load() -> Result<Self> {
        if let Ok(path) = std::env::var(ENV_VAULT_PATH) {
            return Ok(Self {
                vault_path: PathBuf::from(path),
            });
        }

        let proj =
            ProjectDirs::from("", "", APP).ok_or(Error::NoHomeDir)?;

        Ok(Self {
            vault_path: proj.data_dir().join(VAULT_FILE),
        })
    }
}
