//! Vault directory: layout, guarded reads and atomic writes under a lock.

use std::io::{Read as _, Write as _};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use age::secrecy::SecretString;
use zeroize::Zeroizing;

use crate::{Entry, Error, LocalKey, Name, Result, header};
use crate::key::Key;

pub(crate) const IDENTITY_FILE: &str = ".identity.age";
pub(crate) const CHECK_FILE: &str = ".check.age";
const LOCK_FILE: &str = ".lock";
pub(crate) const MAX_SMALL_FILE: u64 = 64 * 1024;
const MAX_ENTRY_FILE: u64 = 64 * 1024 * 1024 + 64 * 1024;

const GITATTRIBUTES: &str = "*.age binary\n";
const GITIGNORE: &str = "*\n!*/\n!*.age\n!.gitignore\n!.gitattributes\n";

/// An open vault directory (the root is canonicalized).
#[derive(Debug)]
pub struct Vault {
    root: PathBuf,
}

impl Vault {
    /// `KS_VAULT` if set, else `~/.ks`.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidInput`] when the home directory cannot be determined.
    pub fn default_path() -> Result<PathBuf> {
        if let Some(custom) = std::env::var_os("KS_VAULT")
            && !custom.is_empty()
        {
            return Ok(PathBuf::from(custom));
        }
        let home = directories::BaseDirs::new()
            .map(|dirs| dirs.home_dir().to_path_buf())
            .ok_or_else(|| Error::InvalidInput("cannot determine home directory".into()))?;
        Ok(home.join(".ks"))
    }

    /// Opens an existing vault (`root` itself may be a symlink).
    ///
    /// # Errors
    ///
    /// - [`Error::NotFound`] when `.identity.age` is missing.
    /// - [`Error::Io`] on filesystem failures.
    pub fn open(root: &Path) -> Result<Self> {
        let root = root.canonicalize().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Error::NotFound(root.display().to_string())
            } else {
                Error::Io {
                    path: root.to_path_buf(),
                    source: e,
                }
            }
        })?;
        let identity = root.join(IDENTITY_FILE);
        if !identity.is_file() {
            return Err(Error::NotFound(identity.display().to_string()));
        }
        Ok(Self { root })
    }

    /// Creates a new vault at `root` (which must not exist or be an empty
    /// directory), sealing a fresh key under `passphrase`.
    ///
    /// # Errors
    ///
    /// - [`Error::WeakPassphrase`] on policy failure.
    /// - [`Error::Exists`] when `root` is a non-empty directory.
    /// - [`Error::Io`] on filesystem failures.
    pub fn create(root: &Path, passphrase: &SecretString) -> Result<Session> {
        crate::passphrase::check_new_passphrase(passphrase)?;
        if root.exists() {
            let is_empty_dir = root.read_dir().is_ok_and(|mut it| it.next().is_none());
            if !is_empty_dir {
                return Err(Error::Exists(root.display().to_string()));
            }
        }
        std::fs::create_dir_all(root).map_err(|e| Error::Io {
            path: root.to_path_buf(),
            source: e,
        })?;
        #[cfg(unix)]
        drop(std::fs::set_permissions(
            root,
            std::fs::Permissions::from_mode(0o700),
        ));
        let vault = Self {
            root: root.canonicalize().map_err(|e| Error::Io {
                path: root.to_path_buf(),
                source: e,
            })?,
        };
        let key = LocalKey::generate();
        let identity_bytes = crate::passphrase::seal_identity(&key, passphrase)?;
        vault.with_write_lock(|| {
            vault.atomic_write(&vault.root.join(IDENTITY_FILE), &identity_bytes)?;
            vault.write_check(&key)?;
            vault.atomic_write(&vault.root.join(".gitattributes"), GITATTRIBUTES.as_bytes())?;
            vault.atomic_write(&vault.root.join(".gitignore"), GITIGNORE.as_bytes())?;
            Ok(())
        })?;
        Ok(Session {
            vault,
            key: Key::Local(key),
            warnings: Vec::new(),
        })
    }

    /// The canonicalized vault root.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Lists entry names without decrypting anything. Dot-files, non-`.age`
    /// files, symlinks and invalid names are skipped. Optionally restricted to
    /// names under `prefix`.
    ///
    /// # Errors
    ///
    /// [`Error::Io`] on filesystem failures.
    pub fn list(&self, prefix: Option<&Name>) -> Result<Vec<Name>> {
        let mut names = Vec::new();
        let mut stack = vec![self.root.clone()];
        while let Some(dir) = stack.pop() {
            let entries = std::fs::read_dir(&dir).map_err(|e| Error::Io {
                path: dir.clone(),
                source: e,
            })?;
            for entry in entries.flatten() {
                self.visit_list_entry(&mut stack, &mut names, &entry, prefix)?;
            }
        }
        names.sort();
        Ok(names)
    }

    /// Handles one directory entry during [`Vault::list`]: pushes
    /// directories onto `stack` and valid entry names onto `names`.
    fn visit_list_entry(
        &self,
        stack: &mut Vec<PathBuf>,
        names: &mut Vec<Name>,
        entry: &std::fs::DirEntry,
        prefix: Option<&Name>,
    ) -> Result<()> {
        let Some(file_name) = entry.file_name().to_str().map(str::to_owned) else {
            return Ok(());
        };
        if file_name.starts_with('.') {
            return Ok(());
        }
        let file_type = entry.file_type().map_err(|e| Error::Io {
            path: entry.path(),
            source: e,
        })?;
        if file_type.is_symlink() {
            return Ok(());
        }
        if file_type.is_dir() {
            stack.push(entry.path());
            return Ok(());
        }
        let entry_path = entry.path();
        let Ok(relative) = entry_path.strip_prefix(&self.root) else {
            return Ok(());
        };
        let Some(entry_name) = Name::from_relative(relative) else {
            return Ok(());
        };
        if prefix.is_none_or(|p| entry_name.is_under(p)) {
            names.push(entry_name);
        }
        Ok(())
    }

    /// Reads a whole vault file after refusing symlinks in every component
    /// and enforcing a size limit. `path` must lie under the vault root.
    pub(crate) fn read_file(&self, path: &Path, max_size: u64) -> Result<Vec<u8>> {
        self.check_no_symlinks(path)?;
        let meta = path
            .symlink_metadata()
            .map_err(|e| io_or_notfound(path, e))?;
        if !meta.is_file() {
            return Err(Error::Corrupt {
                path: path.to_path_buf(),
                reason: "not a regular file",
            });
        }
        if meta.len() > max_size {
            return Err(Error::Corrupt {
                path: path.to_path_buf(),
                reason: "file too large",
            });
        }
        std::fs::read(path).map_err(|e| Error::Io {
            path: path.to_path_buf(),
            source: e,
        })
    }

    /// Refuses symlinks in every component between the vault root and `path`
    /// (inclusive).
    fn check_no_symlinks(&self, path: &Path) -> Result<()> {
        let corrupt = |p: &Path| Error::Corrupt {
            path: p.to_path_buf(),
            reason: "symlinks are not allowed inside the vault",
        };
        let mut cursor = self.root.clone();
        for component in path
            .strip_prefix(&self.root)
            .map_err(|_| Error::Internal("path outside vault"))?
            .components()
        {
            cursor.push(component);
            match cursor.symlink_metadata() {
                Ok(meta) if meta.file_type().is_symlink() => return Err(corrupt(&cursor)),
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => {
                    return Err(Error::Io {
                        path: cursor.clone(),
                        source: e,
                    });
                }
            }
        }
        Ok(())
    }

    /// Checks `.check.age` under `key`: exists, single-X25519, decrypts, and
    /// contains exactly `ks1 check <sha256 of .identity.age>\n`. Every read
    /// goes through [`Vault::read_file`] (symlink refusal, size cap).
    pub(crate) fn verify_check(&self, key: &dyn age::Identity) -> bool {
        self.verify_check_inner(key).unwrap_or(false)
    }

    fn verify_check_inner(&self, key: &dyn age::Identity) -> Option<bool> {
        let identity_bytes = self
            .read_file(&self.root.join(IDENTITY_FILE), MAX_SMALL_FILE)
            .ok()?;
        let expected = format!("ks1 check {}\n", hex_sha256(&identity_bytes));
        let check_path = self.root.join(CHECK_FILE);
        let check_bytes = self.read_file(&check_path, MAX_SMALL_FILE).ok()?;
        header::check_single_x25519(&check_bytes, &check_path).ok()?;
        let decryptor = age::Decryptor::new_buffered(&check_bytes[..]).ok()?;
        let identity: &dyn age::Identity = key;
        let reader = decryptor.decrypt(core::iter::once(identity)).ok()?;
        let mut plaintext = Zeroizing::new(Vec::with_capacity(check_bytes.len()));
        reader.take(1024).read_to_end(&mut plaintext).ok()?;
        Some(*plaintext == *expected.as_bytes())
    }

    /// Encrypts `ks1 check <sha256 of .identity.age>` to the current key and
    /// atomically writes `.check.age`.
    pub(crate) fn write_check(&self, key: &LocalKey) -> Result<()> {
        let identity_bytes = self.read_file(&self.root.join(IDENTITY_FILE), MAX_SMALL_FILE)?;
        let plaintext = format!("ks1 check {}\n", hex_sha256(&identity_bytes));
        let encrypted = encrypt_to(key, Zeroizing::new(plaintext.into_bytes()))?;
        self.atomic_write(&self.root.join(CHECK_FILE), &encrypted)
    }

    /// Runs `f` holding the exclusive write lock on `<root>/.lock`.
    pub(crate) fn with_write_lock<R>(&self, f: impl FnOnce() -> Result<R>) -> Result<R> {
        let path = self.root.join(LOCK_FILE);
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)
            .map_err(|e| Error::Io {
                path: path.clone(),
                source: e,
            })?;
        #[cfg(unix)]
        drop(file.set_permissions(std::fs::Permissions::from_mode(0o600)));
        let mut lock = fd_lock::RwLock::new(file);
        let _guard = lock.write().map_err(|e| Error::Io { path, source: e })?;
        f()
    }

    /// Removes directories below the vault root that became empty after a
    /// deletion. Best-effort: stops at the first non-empty or failing dir.
    /// Caller must hold the write lock.
    fn prune_empty_parents(&self, path: &Path) {
        let mut dir = path.parent();
        while let Some(d) = dir {
            if d == self.root {
                break;
            }
            match std::fs::remove_dir(d) {
                Ok(()) => dir = d.parent(),
                Err(_) => break,
            }
        }
    }

    /// Atomically replaces `path` with `bytes`: `.tmp*` tempfile in the target
    /// directory, write + fsync, rename, then fsync the directory (unix).
    pub(crate) fn atomic_write(&self, path: &Path, bytes: &[u8]) -> Result<()> {
        self.check_no_symlinks(path)?;
        let dir = path.parent().unwrap_or(&self.root);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt as _;
            std::fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(dir)
                .map_err(|e| Error::Io {
                    path: dir.to_path_buf(),
                    source: e,
                })?;
        }
        #[cfg(not(unix))]
        std::fs::create_dir_all(dir).map_err(|e| Error::Io {
            path: dir.to_path_buf(),
            source: e,
        })?;
        let mut tmp = tempfile::Builder::new()
            .prefix(".tmp")
            .tempfile_in(dir)
            .map_err(|e| Error::Io {
                path: path.to_path_buf(),
                source: e,
            })?;
        tmp.write_all(bytes)
            .and_then(|()| tmp.as_file().sync_all())
            .map_err(|e| Error::Io {
                path: path.to_path_buf(),
                source: e,
            })?;
        #[cfg(unix)]
        drop(
            tmp.as_file()
                .set_permissions(std::fs::Permissions::from_mode(0o600)),
        );
        tmp.persist(path).map_err(|e| Error::Io {
            path: path.to_path_buf(),
            source: e.error,
        })?;
        #[cfg(unix)]
        if let Ok(dir_file) = std::fs::File::open(dir) {
            drop(dir_file.sync_all());
        }
        Ok(())
    }
}

fn io_or_notfound(path: &Path, e: std::io::Error) -> Error {
    if e.kind() == std::io::ErrorKind::NotFound {
        Error::NotFound(path.display().to_string())
    } else {
        Error::Io {
            path: path.to_path_buf(),
            source: e,
        }
    }
}

pub(crate) fn hex_sha256(bytes: &[u8]) -> String {
    use sha2::Digest as _;
    crate::hex(sha2::Sha256::digest(bytes).into())
}

/// Encrypts plaintext to the vault's current recipient.
#[allow(
    clippy::needless_pass_by_value,
    reason = "owning the buffer guarantees zeroization on drop"
)]
pub(crate) fn encrypt_to(key: &dyn age::Recipient, plaintext: Zeroizing<Vec<u8>>) -> Result<Vec<u8>> {
    let recipient: &dyn age::Recipient = key;
    let encryptor = age::Encryptor::with_recipients(core::iter::once(recipient))
        .map_err(|_| Error::Internal("age encryption setup failed"))?;
    let mut out = Vec::new();
    let mut writer = encryptor
        .wrap_output(&mut out)
        .map_err(|_| Error::Internal("age wrap failed"))?;
    writer
        .write_all(&plaintext)
        .and_then(|()| writer.finish().map(|_| ()))
        .map_err(|e| Error::Io {
            path: "entry file".into(),
            source: e,
        })?;
    Ok(out)
}

/// Decrypts a size-checked single-X25519 entry file and decodes its plaintext.
pub(crate) fn decrypt_entry(
    key: &dyn age::Identity,
    bytes: &[u8],
    name: &Name,
    path: &Path,
) -> Result<Entry> {
    header::check_single_x25519(bytes, path)?;
    let corrupt = |reason: &'static str| Error::Corrupt {
        path: path.to_path_buf(),
        reason,
    };
    let decryptor = age::Decryptor::new_buffered(bytes).map_err(|_| corrupt("not an age file"))?;
    let identity: &dyn age::Identity = key;
    let reader = decryptor
        .decrypt(core::iter::once(identity))
        .map_err(|_| corrupt("cannot be decrypted with the current key"))?;
    let mut plaintext = Zeroizing::new(Vec::with_capacity(bytes.len()));
    reader
        .take(crate::entry::MAX_ENTRY_PLAINTEXT + 4096)
        .read_to_end(&mut plaintext)
        .map_err(|_| corrupt("decryption failed"))?;
    Entry::decode(plaintext, name, path)
}

/// An unlocked session over a [`Vault`].
#[derive(Debug)]
pub struct Session {
    vault: Vault,
    key: Key,
    warnings: Vec<String>,
}

impl Session {
    pub(crate) const fn new(vault: Vault, key: Key, warnings: Vec<String>) -> Self {
        Self {
            vault,
            key,
            warnings,
        }
    }

    /// Reads and decrypts an entry.
    ///
    /// # Errors
    ///
    /// [`Error::NotFound`] if absent; [`Error::Corrupt`] on hostile or
    /// undecryptable files.
    pub fn get(&self, name: &Name) -> Result<Entry> {
        let path = name.path_in(&self.vault.root);
        let bytes = self.vault.read_file(&path, MAX_ENTRY_FILE)?;
        decrypt_entry(&self.key, &bytes, name, &path)
    }

    /// Writes an entry, atomically and under the write lock.
    ///
    /// # Errors
    ///
    /// [`Error::Exists`] when the entry exists and `overwrite` is false.
    pub fn set(&self, name: &Name, entry: &Entry, overwrite: bool) -> Result<()> {
        let path = name.path_in(&self.vault.root);
        if !overwrite && path.exists() {
            return Err(Error::Exists(name.to_string()));
        }
        self.vault.with_write_lock(|| {
            if !overwrite && path.exists() {
                return Err(Error::Exists(name.to_string()));
            }
            let encrypted = encrypt_to(&self.key, entry.encode(name))?;
            self.vault.atomic_write(&path, &encrypted)
        })
    }

    /// Removes an entry and prunes now-empty parent directories below root.
    ///
    /// # Errors
    ///
    /// [`Error::NotFound`] if absent; [`Error::Corrupt`] on symlinked paths.
    pub fn remove(&self, name: &Name) -> Result<()> {
        let path = name.path_in(&self.vault.root);
        self.vault.check_no_symlinks(&path)?;
        if !path.exists() {
            return Err(Error::NotFound(name.to_string()));
        }
        self.vault.with_write_lock(|| {
            std::fs::remove_file(&path).map_err(|e| Error::Io {
                path: path.clone(),
                source: e,
            })?;
            self.vault.prune_empty_parents(&path);
            Ok(())
        })
    }

    /// Re-encrypts an entry under a new name, then deletes the old file.
    /// A crash can leave at most two readable copies.
    ///
    /// # Errors
    ///
    /// [`Error::NotFound`] if `from` is absent; [`Error::Exists`] when `to`
    /// exists and `overwrite` is false.
    pub fn rename(&self, from: &Name, to: &Name, overwrite: bool) -> Result<()> {
        if from == to {
            return Err(Error::InvalidInput(
                "source and destination are the same entry".into(),
            ));
        }
        let from_path = from.path_in(&self.vault.root);
        let to_path = to.path_in(&self.vault.root);
        self.vault.with_write_lock(|| {
            let bytes = self.vault.read_file(&from_path, MAX_ENTRY_FILE)?;
            let entry = decrypt_entry(&self.key, &bytes, from, &from_path)?;
            if !overwrite && to_path.exists() {
                return Err(Error::Exists(to.to_string()));
            }
            let encrypted = encrypt_to(&self.key, entry.encode(to))?;
            self.vault.atomic_write(&to_path, &encrypted)?;
            std::fs::remove_file(&from_path).map_err(|e| Error::Io {
                path: from_path.clone(),
                source: e,
            })?;
            Ok(())
        })
    }

    /// Re-seals the whole identity file (all keys) under a new passphrase and
    /// rewrites `.check.age`.
    ///
    /// # Errors
    ///
    /// [`Error::WeakPassphrase`] on policy failure.
    pub fn change_passphrase(&self, new_passphrase: &SecretString) -> Result<()> {
        crate::passphrase::check_new_passphrase(new_passphrase)?;
        self.vault.with_write_lock(|| {
            #[cfg_attr(
                not(feature = "agent"),
                allow(
                    clippy::infallible_destructuring_match,
                    reason = "Key::Agent only exists with the agent feature"
                )
            )]
            let key = match &self.key {
                Key::Local(k) => k,
                #[cfg(feature = "agent")]
                Key::Agent(_) => {
                    return Err(Error::InvalidInput(
                        "changing the passphrase needs the master passphrase, Touch ID or \
                         keychain"
                            .into(),
                    ));
                }
            };
            let identity_bytes = crate::passphrase::seal_identity(key, new_passphrase)?;
            self.vault
                .atomic_write(&self.vault.root.join(IDENTITY_FILE), &identity_bytes)?;
            self.vault.write_check(key)
        })
    }

    /// The vault key as identity text, for `ks __agent` over a pipe only.
    /// `None` for non-local keys.
    #[must_use]
    #[cfg_attr(
        not(feature = "agent"),
        allow(
            clippy::unnecessary_wraps,
            reason = "Agent keys return None; Option is the contract"
        )
    )]
    pub fn agent_identity(&self) -> Option<SecretString> {
        match &self.key {
            Key::Local(k) => Some(k.to_identity_text()),
            #[cfg(feature = "agent")]
            Key::Agent(_) => None,
        }
    }

    /// Non-fatal warnings collected during unlock.
    #[must_use]
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// The underlying vault.
    #[must_use]
    pub const fn vault(&self) -> &Vault {
        &self.vault
    }
}
