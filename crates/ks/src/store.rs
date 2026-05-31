//! The encrypted secret store.
//!
//! A [`Store`] is a directory tree where each secret is its own age file
//! (`<store>/<logical/path>.age`) and a top-level `.age-recipients` file lists
//! the X25519 public keys allowed to decrypt it.
//!
//! The API mirrors age's natural asymmetry:
//!
//! - **Writing** ([`set`](Store::set), [`insert`](Store::insert),
//!   [`delete`](Store::delete), [`list`](Store::list)) needs only the recipient
//!   public keys, so it never prompts for a passphrase.
//! - **Reading** ([`get`](Store::get), [`grep`](Store::grep)), moving
//!   ([`rename`](Store::rename), [`copy`](Store::copy)) and rotating recipients
//!   ([`set_recipients`](Store::set_recipients)) require the caller-supplied
//!   [`x25519::Identity`].
//!
//! Each secret is wrapped in a versioned envelope that binds it to its logical
//! path, so moving a secret re-encrypts it under the new path and a relocated or
//! swapped ciphertext file is detected on read.

use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

use age::x25519;
use fd_lock::RwLock;
use zeroize::Zeroizing;

use crate::config::Config;
use crate::crypto;
use crate::envelope;
use crate::error::{Error, Result};
use crate::path as pathutil;
use crate::secret::Secret;

/// Name of the advisory-lock file kept at the store root.
const LOCK_FILE: &str = ".ks.lock";

/// Name of the staging directory used for transactional recipient rotation.
const ROTATE_DIR: &str = ".ks-rotate";

/// An encrypted store bound to a config and its recipient list.
pub struct Store {
    config: Config,
    recipients: Vec<x25519::Recipient>,
}

impl std::fmt::Debug for Store {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Store")
            .field("store_dir", &self.config.store_dir)
            .field("recipients", &self.recipients.len())
            .finish()
    }
}

impl Store {
    /// Opens an existing store and loads its recipients. Does **not** unlock the
    /// identity, so the returned store can write but not yet read secrets.
    ///
    /// # Errors
    /// - [`Error::StoreNotFound`] if the store directory does not exist.
    /// - [`Error::NoRecipients`] if `.age-recipients` is missing or empty.
    /// - [`Error::Io`] / [`Error::InvalidRecipient`] on parse failures.
    pub fn open(config: Config) -> Result<Self> {
        if !config.store_dir.exists() {
            return Err(Error::StoreNotFound(config.store_dir));
        }
        let recipients = crypto::load_recipients(&config.recipients_path())?;
        Ok(Self { config, recipients })
    }

    /// Creates a brand-new store, writing `.age-recipients` with the owner's
    /// public key plus any `extra` recipients.
    ///
    /// # Errors
    /// - [`Error::StoreExists`] if `.age-recipients` already exists.
    /// - [`Error::Io`] on filesystem failures.
    pub fn create(
        config: Config,
        owner: &x25519::Identity,
        extra: &[x25519::Recipient],
    ) -> Result<Self> {
        let recipients_path = config.recipients_path();
        if recipients_path.exists() {
            return Err(Error::StoreExists(config.store_dir));
        }
        crypto::create_dir_all_secure(&config.store_dir)?;

        let mut recipients = Vec::with_capacity(extra.len().saturating_add(1));
        recipients.push(owner.to_public());
        for r in extra {
            if !crypto::recipients_contain(&recipients, r) {
                recipients.push(r.clone());
            }
        }
        crypto::save_recipients(&recipients_path, &recipients)?;
        Ok(Self { config, recipients })
    }

    /// Returns the absolute store directory.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.config.store_dir
    }

    /// Returns the configured recipient list.
    #[must_use]
    pub fn recipients(&self) -> &[x25519::Recipient] {
        &self.recipients
    }

    /// Opens (creating if absent) the store's advisory-lock file.
    fn lock_file(&self) -> Result<File> {
        OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(self.config.store_dir.join(LOCK_FILE))
            .map_err(Error::Io)
    }

    /// Runs `f` while holding an exclusive advisory lock on the store, so two
    /// `ks` processes never mutate it concurrently. Reads do not lock: every
    /// write lands via an atomic rename, so a concurrent reader always sees
    /// either the old file or the new one, never a partial write.
    fn with_write_lock<T>(&self, f: impl FnOnce() -> Result<T>) -> Result<T> {
        let mut lock = RwLock::new(self.lock_file()?);
        let _guard = lock.write().map_err(Error::Io)?;
        f()
    }

    /// Returns `true` if a secret exists at `logical`.
    #[must_use]
    pub fn exists(&self, logical: &str) -> bool {
        pathutil::validate(logical).is_ok()
            && pathutil::to_file(&self.config.store_dir, logical).is_file()
    }

    /// Encrypts and writes (or overwrites) `secret` at `logical`.
    ///
    /// # Errors
    /// [`Error::InvalidPath`] for malformed paths; [`Error::Io`] /
    /// [`Error::Encrypt`] on failure.
    pub fn set(&self, logical: &str, secret: &Secret) -> Result<()> {
        pathutil::validate(logical)?;
        self.with_write_lock(|| self.write_secret(logical, secret))
    }

    /// Encrypts and writes `secret` at `logical` *without* taking the store
    /// lock; callers must already hold it via [`with_write_lock`](Store::with_write_lock).
    fn write_secret(&self, logical: &str, secret: &Secret) -> Result<()> {
        let wrapped = envelope::wrap(logical, secret.kind(), secret.as_bytes());
        let ciphertext = crypto::encrypt(&wrapped, &self.recipients)?;
        crypto::write_atomic(
            &pathutil::to_file(&self.config.store_dir, logical),
            &ciphertext,
        )
    }

    /// Inserts a new secret, failing with [`Error::SecretExists`] if present.
    ///
    /// # Errors
    /// See [`set`](Store::set) plus [`Error::SecretExists`].
    pub fn insert(&self, logical: &str, secret: &Secret) -> Result<()> {
        pathutil::validate(logical)?;
        self.with_write_lock(|| {
            if self.exists(logical) {
                return Err(Error::SecretExists(logical.to_owned()));
            }
            self.write_secret(logical, secret)
        })
    }

    /// Reads and decrypts the secret at `logical`.
    ///
    /// # Errors
    /// [`Error::InvalidPath`], [`Error::SecretNotFound`], or [`Error::Decrypt`]
    /// / [`Error::Io`] on failure.
    pub fn get(&self, logical: &str, identity: &x25519::Identity) -> Result<Secret> {
        pathutil::validate(logical)?;
        let file = pathutil::to_file(&self.config.store_dir, logical);
        if !file.exists() {
            return Err(Error::SecretNotFound(logical.to_owned()));
        }
        let plaintext = crypto::decrypt(&std::fs::read(&file)?, identity)?;
        let (kind, payload) = envelope::unwrap(logical, &plaintext)?;
        Ok(Secret::from_bytes(payload, kind))
    }

    /// Deletes the secret at `logical`, pruning now-empty parent directories.
    ///
    /// # Errors
    /// [`Error::SecretNotFound`] if the file is absent; [`Error::Io`] otherwise.
    pub fn delete(&self, logical: &str) -> Result<()> {
        pathutil::validate(logical)?;
        self.with_write_lock(|| {
            let file = pathutil::to_file(&self.config.store_dir, logical);
            if !file.exists() {
                return Err(Error::SecretNotFound(logical.to_owned()));
            }
            std::fs::remove_file(&file)?;
            prune_empty_parents(&self.config.store_dir, file.parent());
            Ok(())
        })
    }

    /// Renames a secret: decrypts it, re-binds the envelope to `to`, re-encrypts,
    /// writes the destination, then removes the source. Needs the identity
    /// because the path binding lives inside the ciphertext.
    ///
    /// # Errors
    /// [`Error::SecretNotFound`] if `from` is absent, [`Error::SecretExists`] if
    /// `to` exists, [`Error::InvalidPath`] for malformed paths, or
    /// [`Error::Decrypt`] / [`Error::Tampered`] reading the source.
    pub fn rename(&self, from: &str, to: &str, identity: &x25519::Identity) -> Result<()> {
        self.with_write_lock(|| {
            let (src, dst) = self.relocate_paths(from, to)?;
            self.reencrypt_to(&src, from, to, &dst, identity)?;
            std::fs::remove_file(&src)?;
            prune_empty_parents(&self.config.store_dir, src.parent());
            Ok(())
        })
    }

    /// Copies a secret: decrypts it, re-binds the envelope to `to`, re-encrypts,
    /// and writes the destination. Needs the identity for the same reason as
    /// [`rename`](Store::rename).
    ///
    /// # Errors
    /// Same as [`rename`](Store::rename), minus pruning.
    pub fn copy(&self, from: &str, to: &str, identity: &x25519::Identity) -> Result<()> {
        self.with_write_lock(|| {
            let (src, dst) = self.relocate_paths(from, to)?;
            self.reencrypt_to(&src, from, to, &dst, identity)
        })
    }

    /// Decrypts the secret at `src` (bound to `from`), re-wraps it bound to `to`,
    /// re-encrypts to the store's recipients, and writes `dst`. Shared by
    /// [`rename`](Store::rename) and [`copy`](Store::copy).
    fn reencrypt_to(
        &self,
        src: &Path,
        from: &str,
        to: &str,
        dst: &Path,
        identity: &x25519::Identity,
    ) -> Result<()> {
        let plaintext = crypto::decrypt(&std::fs::read(src)?, identity)?;
        let (kind, payload) = envelope::unwrap(from, &plaintext)?;
        let payload = Zeroizing::new(payload);
        let wrapped = envelope::wrap(to, kind, &payload);
        let ciphertext = crypto::encrypt(&wrapped, &self.recipients)?;
        crypto::write_atomic(dst, &ciphertext)
    }

    /// Lists logical paths under `prefix` (`""` for all), sorted.
    ///
    /// # Errors
    /// [`Error::Io`] on directory traversal failures.
    pub fn list(&self, prefix: &str) -> Result<Vec<String>> {
        let mut out = Vec::new();
        walk(&self.config.store_dir, &self.config.store_dir, &mut out)?;
        out.sort();
        if prefix.is_empty() {
            return Ok(out);
        }
        let scope = format!("{prefix}/");
        Ok(out
            .into_iter()
            .filter(|p| p == prefix || p.starts_with(&scope))
            .collect())
    }

    /// Searches paths (always) and decrypted contents (when `identity` is
    /// `Some`) case-insensitively for `query`.
    ///
    /// # Errors
    /// [`Error::Io`] / [`Error::Decrypt`] on failure when scanning contents.
    pub fn grep(&self, query: &str, identity: Option<&x25519::Identity>) -> Result<Vec<String>> {
        let needle = query.to_lowercase();
        let mut hits = Vec::new();
        for path in self.list("")? {
            if path.to_lowercase().contains(&needle) {
                hits.push(path);
                continue;
            }
            if let Some(id) = identity
                && let Ok(secret) = self.get(&path, id)
                && secret.expose().to_lowercase().contains(&needle)
            {
                hits.push(path);
            }
        }
        Ok(hits)
    }

    /// Replaces the recipient list and re-encrypts every secret to it.
    ///
    /// `new_recipients` must include `identity`'s public key, otherwise the user
    /// would lock themselves out.
    ///
    /// Rotation is two-phase: every secret is first re-encrypted into a staging
    /// directory, and only once *all* succeed are the staged files moved over the
    /// live ones (the recipients file last). A failure during preparation (e.g. a
    /// tampered secret) therefore leaves the live store completely untouched. A
    /// crash during the short commit phase can leave a mix of old/new secrets, but
    /// every file is always a complete ciphertext the identity can decrypt, so
    /// re-running converges.
    ///
    /// # Errors
    /// [`Error::InvalidRecipient`] if the user's own key is missing, or
    /// [`Error::Io`] / [`Error::Decrypt`] during re-encryption.
    pub fn set_recipients(
        &mut self,
        new_recipients: Vec<x25519::Recipient>,
        identity: &x25519::Identity,
    ) -> Result<usize> {
        if !crypto::recipients_contain(&new_recipients, &identity.to_public()) {
            return Err(Error::InvalidRecipient(
                "recipient list must include your own public key".into(),
            ));
        }
        let mut lock = RwLock::new(self.lock_file()?);
        let _guard = lock.write().map_err(Error::Io)?;

        let paths = self.list("")?;
        let staging = self.config.store_dir.join(ROTATE_DIR);
        remove_staging(&staging);

        // Phase 1 — prepare: re-encrypt every secret into the staging area. Any
        // failure here leaves the live store untouched.
        if let Err(e) = self.stage_rotation(&paths, &staging, &new_recipients, identity) {
            remove_staging(&staging);
            return Err(e);
        }

        // Phase 2 — commit: move each staged file over its live counterpart,
        // flip the recipients file last, then drop the staging area.
        for path in &paths {
            crypto::rename_replace(
                &pathutil::to_file(&staging, path),
                &pathutil::to_file(&self.config.store_dir, path),
            )?;
        }
        crypto::save_recipients(&self.config.recipients_path(), &new_recipients)?;
        remove_staging(&staging);

        self.recipients = new_recipients;
        Ok(paths.len())
    }

    /// Re-encrypts every secret in `paths` to `new_recipients`, writing the
    /// ciphertext into the `staging` mirror tree. Phase 1 of [`set_recipients`].
    fn stage_rotation(
        &self,
        paths: &[String],
        staging: &Path,
        new_recipients: &[x25519::Recipient],
        identity: &x25519::Identity,
    ) -> Result<()> {
        for path in paths {
            let secret = self.get(path, identity)?;
            let wrapped = envelope::wrap(path, secret.kind(), secret.as_bytes());
            let ciphertext = crypto::encrypt(&wrapped, new_recipients)?;
            crypto::write_atomic(&pathutil::to_file(staging, path), &ciphertext)?;
        }
        Ok(())
    }

    /// Validates and resolves a `from`/`to` pair for [`rename`]/[`copy`],
    /// enforcing that `from` exists and `to` does not.
    fn relocate_paths(&self, from: &str, to: &str) -> Result<(PathBuf, PathBuf)> {
        pathutil::validate(from)?;
        pathutil::validate(to)?;
        let src = pathutil::to_file(&self.config.store_dir, from);
        if !src.exists() {
            return Err(Error::SecretNotFound(from.to_owned()));
        }
        if self.exists(to) {
            return Err(Error::SecretExists(to.to_owned()));
        }
        let dst = pathutil::to_file(&self.config.store_dir, to);
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok((src, dst))
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
            if entry.file_name().to_string_lossy().starts_with('.') {
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

/// Best-effort removal of the rotation staging directory and its contents.
fn remove_staging(dir: &Path) {
    if dir.exists() {
        std::fs::remove_dir_all(dir).ok();
    }
}

#[cfg(test)]
mod tests {
    use age::secrecy::SecretString;

    use super::*;

    fn fresh() -> (Config, x25519::Identity) {
        let root = std::env::temp_dir().join(format!("ks-store-{}", rand::random::<u64>()));
        std::fs::create_dir_all(&root).expect("temp");
        let cfg = Config {
            identity_path: root.join("identity.age"),
            store_dir: root.join("store"),
        };
        let id = crypto::create_identity(&cfg.identity_path, SecretString::from("pw".to_owned()))
            .expect("identity");
        (cfg, id)
    }

    #[test]
    fn set_needs_no_identity_get_does() {
        let (cfg, id) = fresh();
        let store = Store::create(cfg, &id, &[]).expect("create");
        store
            .set("github/token", &Secret::new("ghp_xxx\nuser: alice\n"))
            .expect("set");
        let got = store.get("github/token", &id).expect("get");
        assert_eq!(got.password(), "ghp_xxx");
        assert_eq!(got.get("user"), Some("alice"));
        assert_eq!(
            store.list("").expect("list"),
            vec!["github/token".to_owned()]
        );
    }

    #[test]
    fn rename_and_copy_rebind_path() {
        let (cfg, id) = fresh();
        let store = Store::create(cfg, &id, &[]).expect("create");
        store.set("a/b", &Secret::new("v")).expect("set");

        store.copy("a/b", "a/c", &id).expect("copy");
        assert!(store.exists("a/b") && store.exists("a/c"));
        assert_eq!(store.get("a/c", &id).expect("get").password(), "v");

        store.rename("a/b", "x/y", &id).expect("rename");
        assert!(!store.exists("a/b") && store.exists("x/y"));
        assert_eq!(store.get("x/y", &id).expect("get").password(), "v");
    }

    #[test]
    fn relocating_ciphertext_is_detected_as_tampering() {
        let (cfg, id) = fresh();
        let store = Store::create(cfg.clone(), &id, &[]).expect("create");
        store.set("a", &Secret::new("secret-a")).expect("a");
        store.set("b", &Secret::new("secret-b")).expect("b");

        // Swap the two ciphertext files behind the store's back.
        let pa = pathutil::to_file(&cfg.store_dir, "a");
        let pb = pathutil::to_file(&cfg.store_dir, "b");
        let tmp = cfg.store_dir.join("swap.tmp");
        std::fs::rename(&pa, &tmp).expect("mv a");
        std::fs::rename(&pb, &pa).expect("mv b->a");
        std::fs::rename(&tmp, &pb).expect("mv tmp->b");

        // Reading `a` now decrypts b's payload, whose bound path is `b`.
        assert!(matches!(store.get("a", &id), Err(Error::Tampered { .. })));
    }

    #[test]
    fn binary_secret_roundtrips_through_store() {
        let (cfg, id) = fresh();
        let store = Store::create(cfg, &id, &[]).expect("create");
        let raw = vec![0u8, b'\n', 0xff, 0x00, b'x'];
        store
            .set(
                "certs/key",
                &Secret::from_bytes(raw.clone(), crate::secret::SecretKind::Binary),
            )
            .expect("set binary");
        let got = store.get("certs/key", &id).expect("get");
        assert!(got.is_binary());
        assert_eq!(got.as_bytes(), &raw[..]);
    }

    #[test]
    fn grep_paths_then_values() {
        let (cfg, id) = fresh();
        let store = Store::create(cfg, &id, &[]).expect("create");
        store.set("github/token", &Secret::new("ghp")).expect("s1");
        store
            .set("aws/key", &Secret::new("secret\nregion: eu-west-1\n"))
            .expect("s2");

        assert_eq!(
            store.grep("github", None).expect("grep"),
            vec!["github/token"]
        );
        assert!(store.grep("eu-west", None).expect("grep").is_empty());
        assert_eq!(
            store.grep("eu-west", Some(&id)).expect("grep values"),
            vec!["aws/key"]
        );
    }

    #[test]
    fn set_recipients_reencrypts_and_guards_lockout() {
        let (cfg, id) = fresh();
        let mut store = Store::create(cfg, &id, &[]).expect("create");
        store.set("k", &Secret::new("v")).expect("set");

        let backup = x25519::Identity::generate();
        let n = store
            .set_recipients(vec![id.to_public(), backup.to_public()], &id)
            .expect("reencrypt");
        assert_eq!(n, 1);
        assert_eq!(store.get("k", &id).expect("get").password(), "v");

        let stranger = x25519::Identity::generate();
        assert!(matches!(
            store.set_recipients(vec![stranger.to_public()], &id),
            Err(Error::InvalidRecipient(_))
        ));
    }

    #[test]
    fn failed_rotation_leaves_store_unchanged() {
        let (cfg, id) = fresh();
        let mut store = Store::create(cfg.clone(), &id, &[]).expect("create");
        store.set("a", &Secret::new("va")).expect("a");
        store.set("b", &Secret::new("vb")).expect("b");

        // Corrupt `b` so phase-1 preparation fails when it is decrypted.
        std::fs::write(pathutil::to_file(&cfg.store_dir, "b"), b"garbage").expect("corrupt");
        let a_before = std::fs::read(pathutil::to_file(&cfg.store_dir, "a")).expect("read a");

        let backup = x25519::Identity::generate();
        assert!(
            store
                .set_recipients(vec![id.to_public(), backup.to_public()], &id)
                .is_err()
        );

        // `a` ciphertext is byte-for-byte unchanged and the staging area is gone.
        let a_after = std::fs::read(pathutil::to_file(&cfg.store_dir, "a")).expect("read a");
        assert_eq!(
            a_before, a_after,
            "live store must be untouched on rollback"
        );
        assert!(!cfg.store_dir.join(".ks-rotate").exists());
        assert_eq!(store.get("a", &id).expect("get a").password(), "va");
    }
}
