//! The encrypted secret store.
//!
//! A [`Store`] is a directory tree where each secret lives in its own
//! age-encrypted file (`<store>/<logical/path>.age`) and a top-level
//! `.recipients` file lists the X25519 public keys that may decrypt it.
//!
//! Encryption requires only the loaded recipients; decryption requires the
//! caller-supplied [`x25519::Identity`]. Operations are atomic per-file
//! (tmp + fsync + rename).

use std::io::Write as _;
use std::path::{Path, PathBuf};

use age::x25519;

use crate::config::Config;
use crate::crypto;
use crate::error::{Error, Result};
use crate::path as pathutil;
use crate::recipient;
use crate::secret::{Secret, Wire};

/// The encrypted store, bound to a config, a recipient list and an unlocked identity.
pub struct Store {
    config: Config,
    recipients: Vec<x25519::Recipient>,
    identity: x25519::Identity,
}

impl std::fmt::Debug for Store {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Store")
            .field("store_dir", &self.config.store_dir)
            .field("recipients", &self.recipients.len())
            .field("identity", &"<redacted>")
            .finish()
    }
}

impl Store {
    /// Opens an existing store and validates its recipients.
    ///
    /// # Errors
    /// - [`Error::StoreNotFound`] if `store_dir` does not exist.
    /// - [`Error::NoRecipients`] if `.recipients` is missing or empty.
    /// - [`Error::Io`] / [`Error::InvalidRecipient`] on parse failures.
    pub fn open(config: Config, identity: x25519::Identity) -> Result<Self> {
        if !config.store_dir.exists() {
            return Err(Error::StoreNotFound(config.store_dir));
        }
        let recipients = recipient::load(&config.recipients_path())?;
        Ok(Self {
            config,
            recipients,
            identity,
        })
    }

    /// Creates a brand-new store: `store_dir/`, `.recipients`, and an empty tree.
    ///
    /// `identity` is the just-created user identity. Its public key plus any
    /// `extra_recipients` are written into `.recipients`.
    ///
    /// # Errors
    /// - [`Error::StoreExists`] if `.recipients` already exists in `store_dir`.
    /// - [`Error::Io`] on filesystem failures.
    pub fn create(
        config: Config,
        identity: x25519::Identity,
        extra_recipients: &[x25519::Recipient],
    ) -> Result<Self> {
        let recipients_path = config.recipients_path();
        if recipients_path.exists() {
            return Err(Error::StoreExists(config.store_dir));
        }
        std::fs::create_dir_all(&config.store_dir)?;

        let mut recipients = Vec::with_capacity(extra_recipients.len().saturating_add(1));
        recipients.push(identity.to_public());
        for r in extra_recipients {
            if !recipient::contains(&recipients, r) {
                recipients.push(r.clone());
            }
        }
        recipient::save(&recipients_path, &recipients)?;

        Ok(Self {
            config,
            recipients,
            identity,
        })
    }

    /// Returns the absolute store directory.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.config.store_dir
    }

    /// Returns the unlocked identity.
    #[must_use]
    pub const fn identity(&self) -> &x25519::Identity {
        &self.identity
    }

    /// Returns the configured recipient list.
    #[must_use]
    pub fn recipients(&self) -> &[x25519::Recipient] {
        &self.recipients
    }

    /// Returns `true` if a secret exists at `logical`.
    #[must_use]
    pub fn exists(&self, logical: &str) -> bool {
        pathutil::validate(logical).is_ok()
            && pathutil::to_file(&self.config.store_dir, logical).is_file()
    }

    /// Reads and decrypts the secret at `logical`.
    ///
    /// # Errors
    /// - [`Error::InvalidPath`] for malformed paths.
    /// - [`Error::SecretNotFound`] if no such file exists.
    /// - [`Error::Decrypt`] / [`Error::Io`] on failure.
    pub fn get(&self, logical: &str) -> Result<Secret> {
        pathutil::validate(logical)?;
        let file = pathutil::to_file(&self.config.store_dir, logical);
        if !file.exists() {
            return Err(Error::SecretNotFound(logical.to_owned()));
        }
        let ciphertext = std::fs::read(&file)?;
        let plaintext = crypto::decrypt_with_identity(&ciphertext, &self.identity)?;
        let wire: Wire = serde_json::from_slice(&plaintext)?;
        Ok(wire.into())
    }

    /// Encrypts and writes (or overwrites) `secret` at `logical`.
    ///
    /// # Errors
    /// - [`Error::InvalidPath`] for malformed paths.
    /// - [`Error::Io`] / [`Error::Encrypt`] on failure.
    pub fn set(&self, logical: &str, secret: &Secret) -> Result<()> {
        pathutil::validate(logical)?;
        let wire = Wire::from(secret);
        let plaintext = serde_json::to_vec(&wire)?;
        let ciphertext = crypto::encrypt_to_recipients(&plaintext, &self.recipients)?;
        let file = pathutil::to_file(&self.config.store_dir, logical);
        write_atomic(&file, &ciphertext)?;
        Ok(())
    }

    /// Inserts a new secret, failing if one already exists.
    ///
    /// # Errors
    /// Returns [`Error::SecretExists`] if a file is already present.
    pub fn insert(&self, logical: &str, secret: &Secret) -> Result<()> {
        if self.exists(logical) {
            return Err(Error::SecretExists(logical.to_owned()));
        }
        self.set(logical, secret)
    }

    /// Deletes the secret at `logical`. Also prunes now-empty parent
    /// directories up to (but not including) the store root.
    ///
    /// # Errors
    /// Returns [`Error::SecretNotFound`] if the file is absent.
    pub fn delete(&self, logical: &str) -> Result<()> {
        pathutil::validate(logical)?;
        let file = pathutil::to_file(&self.config.store_dir, logical);
        if !file.exists() {
            return Err(Error::SecretNotFound(logical.to_owned()));
        }
        std::fs::remove_file(&file)?;
        prune_empty_parents(&self.config.store_dir, file.parent());
        Ok(())
    }

    /// Renames a secret from one logical path to another.
    ///
    /// # Errors
    /// - [`Error::SecretNotFound`] if `from` does not exist.
    /// - [`Error::SecretExists`] if `to` already exists.
    /// - [`Error::InvalidPath`] for malformed `to`.
    pub fn rename(&self, from: &str, to: &str) -> Result<()> {
        pathutil::validate(from)?;
        pathutil::validate(to)?;
        if self.exists(to) {
            return Err(Error::SecretExists(to.to_owned()));
        }
        let src = pathutil::to_file(&self.config.store_dir, from);
        if !src.exists() {
            return Err(Error::SecretNotFound(from.to_owned()));
        }
        let dst = pathutil::to_file(&self.config.store_dir, to);
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::rename(&src, &dst)?;
        prune_empty_parents(&self.config.store_dir, src.parent());
        Ok(())
    }

    /// Lists logical paths under `prefix` (use `""` for all), sorted.
    ///
    /// # Errors
    /// Returns [`Error::Io`] on directory traversal failures.
    pub fn list(&self, prefix: &str) -> Result<Vec<String>> {
        let mut out = Vec::new();
        walk(&self.config.store_dir, &self.config.store_dir, &mut out)?;
        out.sort();
        if prefix.is_empty() {
            Ok(out)
        } else {
            Ok(out
                .into_iter()
                .filter(|p| p == prefix || p.starts_with(&format!("{prefix}/")))
                .collect())
        }
    }

    /// Searches paths and decrypted notes case-insensitively for `query`.
    ///
    /// Notes are scanned only when `include_notes` is `true` (which requires
    /// decrypting every secret — slow on large stores).
    ///
    /// # Errors
    /// Returns [`Error::Io`] / [`Error::Decrypt`] on failure when scanning notes.
    pub fn find(&self, query: &str, include_notes: bool) -> Result<Vec<String>> {
        let q = query.to_lowercase();
        let all = self.list("")?;
        let mut hits = Vec::new();
        for path in all {
            if path.to_lowercase().contains(&q) {
                hits.push(path);
                continue;
            }
            if include_notes
                && let Ok(s) = self.get(&path)
                && s.note.to_lowercase().contains(&q)
            {
                hits.push(path);
            }
        }
        Ok(hits)
    }

    /// Replaces the recipient list and re-encrypts every secret.
    ///
    /// `new_recipients` must contain at least the current user's public key
    /// (otherwise we'd lock ourselves out — checked).
    ///
    /// # Errors
    /// Returns [`Error::InvalidRecipient`] if the resulting list does not
    /// include the user's own public key, or [`Error::Io`] / [`Error::Decrypt`]
    /// during re-encryption.
    pub fn set_recipients(&mut self, new_recipients: Vec<x25519::Recipient>) -> Result<usize> {
        let own = self.identity.to_public();
        if !recipient::contains(&new_recipients, &own) {
            return Err(Error::InvalidRecipient(
                "recipient list must include your own public key".into(),
            ));
        }
        let paths = self.list("")?;
        for path in &paths {
            let secret = self.get(path)?;
            let wire = Wire::from(&secret);
            let plaintext = serde_json::to_vec(&wire)?;
            let ciphertext = crypto::encrypt_to_recipients(&plaintext, &new_recipients)?;
            let file = pathutil::to_file(&self.config.store_dir, path);
            write_atomic(&file, &ciphertext)?;
        }
        recipient::save(&self.config.recipients_path(), &new_recipients)?;
        self.recipients = new_recipients;
        Ok(paths.len())
    }
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let entry_path = entry.path();

        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with('.') {
                continue;
            }
            walk(root, &entry_path, out)?;
            continue;
        }
        if let Some(logical) = pathutil::from_file(root, &entry_path) {
            out.push(logical);
        }
    }
    Ok(())
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("age.tmp");
    {
        let mut file = std::fs::File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    set_owner_only(&tmp)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(unix)]
fn set_owner_only(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    let mut perms = std::fs::metadata(path)?.permissions();
    perms.set_mode(0o600);
    std::fs::set_permissions(path, perms)?;
    Ok(())
}

#[cfg(not(unix))]
#[expect(
    clippy::unnecessary_wraps,
    reason = "signature parity with the Unix impl that genuinely needs Result"
)]
const fn set_owner_only(_path: &Path) -> Result<()> {
    Ok(())
}

fn prune_empty_parents(root: &Path, dir: Option<&Path>) {
    let Some(mut cur) = dir else { return };
    let mut owned: PathBuf;
    while cur != root {
        let Ok(mut entries) = std::fs::read_dir(cur) else {
            return;
        };
        if entries.next().is_some() {
            return;
        }
        if std::fs::remove_dir(cur).is_err() {
            return;
        }
        let Some(parent) = cur.parent() else { return };
        owned = parent.to_path_buf();
        cur = &owned;
    }
}

#[cfg(test)]
mod tests {
    use age::secrecy::SecretString;

    use super::*;
    use crate::identity;
    use crate::secret::Secret;

    fn tempdir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ks-store-{}", rand::random::<u64>()));
        std::fs::create_dir_all(&dir).expect("temp");
        dir
    }

    fn fresh_config() -> (Config, x25519::Identity) {
        let root = tempdir();
        let cfg = Config {
            identity_path: root.join("identity.age"),
            store_dir: root.join("store"),
            config_path: root.join("config.toml"),
            tunables: crate::Tunables::default(),
        };
        let pp = SecretString::from("pw".to_owned());
        let id = identity::create(&cfg.identity_path, pp).expect("create identity");
        (cfg, id)
    }

    #[test]
    fn create_then_set_get_delete() {
        let (cfg, id) = fresh_config();
        let store = Store::create(cfg, id, &[]).expect("create store");

        let s = Secret::new("v1").with_note("note");
        store.set("github/token", &s).expect("set");
        let g = store.get("github/token").expect("get");
        assert_eq!(&*g.value, "v1");
        assert_eq!(g.note, "note");

        let listed = store.list("").expect("list");
        assert_eq!(listed, vec!["github/token".to_owned()]);

        store.delete("github/token").expect("delete");
        assert!(!store.exists("github/token"));
    }

    #[test]
    fn rename_works() {
        let (cfg, id) = fresh_config();
        let store = Store::create(cfg, id, &[]).expect("create");
        store.set("a/b", &Secret::new("v")).expect("set");
        store.rename("a/b", "x/y").expect("rename");
        assert!(!store.exists("a/b"));
        assert!(store.exists("x/y"));
    }

    #[test]
    fn set_recipients_reencrypts_all() {
        let (cfg, id) = fresh_config();
        let mut store = Store::create(cfg, id, &[]).expect("create");
        store.set("k1", &Secret::new("v1")).expect("set 1");
        store.set("k2", &Secret::new("v2")).expect("set 2");

        let backup = x25519::Identity::generate();
        let new_list = vec![store.identity().to_public(), backup.to_public()];
        let n = store.set_recipients(new_list).expect("reencrypt");
        assert_eq!(n, 2);

        // Original identity still works.
        assert_eq!(&*store.get("k1").expect("get").value, "v1");
    }

    #[test]
    fn refuses_to_lock_out_user() {
        let (cfg, id) = fresh_config();
        let mut store = Store::create(cfg, id, &[]).expect("create");
        let stranger = x25519::Identity::generate();
        let err = store
            .set_recipients(vec![stranger.to_public()])
            .expect_err("must refuse");
        assert!(matches!(err, Error::InvalidRecipient(_)));
    }
}
