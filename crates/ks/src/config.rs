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

    /// Reports filesystem-permission problems with the identity, store directory
    /// and recipients file (each checked only if it exists).
    ///
    /// On Unix this flags any path accessible by group or other; on other
    /// platforms it returns an empty list, since Windows uses ACLs rather than
    /// mode bits and is out of scope.
    #[cfg_attr(
        not(unix),
        allow(
            clippy::unused_self,
            clippy::missing_const_for_fn,
            reason = "non-Unix has no mode bits to inspect; parity with the Unix impl"
        )
    )]
    #[must_use]
    pub fn permission_issues(&self) -> Vec<String> {
        #[cfg(unix)]
        {
            let mut issues = Vec::new();
            check_mode(&self.identity_path, "identity file", 0o600, &mut issues);
            check_mode(&self.store_dir, "store directory", 0o700, &mut issues);
            check_mode(
                &self.recipients_path(),
                "recipients file",
                0o600,
                &mut issues,
            );
            issues
        }
        #[cfg(not(unix))]
        {
            Vec::new()
        }
    }
}

fn env_path(name: &str) -> Option<PathBuf> {
    match std::env::var(name) {
        Ok(raw) if !raw.is_empty() => Some(PathBuf::from(raw)),
        _ => None,
    }
}

/// Pushes an issue if `path` exists and is accessible by group or other.
#[cfg(unix)]
fn check_mode(path: &std::path::Path, kind: &str, want: u32, issues: &mut Vec<String>) {
    use std::os::unix::fs::PermissionsExt as _;
    let Ok(meta) = std::fs::metadata(path) else {
        return;
    };
    let mode = meta.permissions().mode() & 0o777;
    if mode & 0o077 != 0 {
        issues.push(format!(
            "{kind} {} is group/other-accessible (mode {mode:03o}); run `chmod {want:o} {}`",
            path.display(),
            path.display()
        ));
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

    #[cfg(unix)]
    #[test]
    fn flags_group_accessible_identity() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = std::env::temp_dir().join(format!("ks-perm-{}", rand::random::<u64>()));
        std::fs::create_dir_all(&dir).expect("temp");
        let identity_path = dir.join("identity.age");
        std::fs::write(&identity_path, b"x").expect("write");
        std::fs::set_permissions(&identity_path, std::fs::Permissions::from_mode(0o644))
            .expect("chmod");
        let cfg = Config {
            identity_path,
            store_dir: dir.join("store"),
        };
        assert!(
            cfg.permission_issues()
                .iter()
                .any(|s| s.contains("identity")),
            "a 0644 identity file must be flagged"
        );
    }
}
