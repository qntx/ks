use std::collections::HashMap;
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};

use zeroize::Zeroizing;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::store::{Secret, SecretData, VaultData};

const VAULT_VERSION: u32 = 1;

/// The in-memory representation of the encrypted vault.
///
/// All secrets are held in plaintext in RAM; the encrypted form is persisted on
/// disk only when [`Vault::save`] is called.
#[derive(Debug)]
pub struct Vault {
    vault_path: PathBuf,
    passphrase: Zeroizing<String>,
    version: u32,
    secrets: HashMap<String, Secret>,
}

impl Vault {
    /// Creates a new, empty vault at the path specified in `config`.
    ///
    /// # Errors
    /// - [`Error::VaultExists`] if a vault file already exists.
    /// - [`Error::Io`] / [`Error::Encrypt`] on write failures.
    pub fn create(config: &Config, passphrase: Zeroizing<String>) -> Result<Self> {
        if config.vault_path.exists() {
            return Err(Error::VaultExists(config.vault_path.clone()));
        }
        let vault = Self {
            vault_path: config.vault_path.clone(),
            passphrase,
            version: VAULT_VERSION,
            secrets: HashMap::new(),
        };
        vault.save()?;
        Ok(vault)
    }

    /// Opens an existing vault, decrypting it with the given passphrase.
    ///
    /// # Errors
    /// - [`Error::VaultNotFound`] if no vault file exists.
    /// - [`Error::WrongPassphrase`] if decryption fails due to bad passphrase.
    /// - [`Error::Io`] / [`Error::Decrypt`] / [`Error::Json`] on read failures.
    pub fn open(config: &Config, passphrase: Zeroizing<String>) -> Result<Self> {
        if !config.vault_path.exists() {
            return Err(Error::VaultNotFound);
        }
        let ciphertext = std::fs::read(&config.vault_path)?;
        let plaintext = age_decrypt(&ciphertext, &passphrase)?;
        let data: VaultData = serde_json::from_slice(&plaintext)?;
        let secrets = data
            .secrets
            .into_iter()
            .map(|(k, v)| (k, Secret::from(v)))
            .collect();
        Ok(Self {
            vault_path: config.vault_path.clone(),
            passphrase,
            version: data.version,
            secrets,
        })
    }

    /// Atomically writes the encrypted vault to disk.
    ///
    /// Uses a `.tmp` file + `fsync` + `rename` pattern to prevent data loss on
    /// power failure.
    ///
    /// # Errors
    /// Returns [`Error::Io`] or [`Error::Encrypt`] on failure.
    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.vault_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let data = VaultData {
            version: self.version,
            secrets: self
                .secrets
                .iter()
                .map(|(k, v)| (k.clone(), SecretData::from(v)))
                .collect(),
        };
        let json = serde_json::to_vec(&data)?;
        let encrypted = age_encrypt(&json, &self.passphrase)?;

        let tmp = self.vault_path.with_extension("age.tmp");
        {
            let mut f = std::fs::File::create(&tmp)?;
            f.write_all(&encrypted)?;
            f.sync_all()?;
        }
        std::fs::rename(&tmp, &self.vault_path)?;
        Ok(())
    }

    /// Returns a reference to the secret at `path`.
    ///
    /// # Errors
    /// Returns [`Error::SecretNotFound`] when `path` is not present.
    pub fn get(&self, path: &str) -> Result<&Secret> {
        self.secrets
            .get(path)
            .ok_or_else(|| Error::SecretNotFound(path.to_owned()))
    }

    /// Inserts or replaces the secret at `path`.
    ///
    /// # Errors
    /// Returns [`Error::InvalidPath`] for empty or invalid paths.
    pub fn set(&mut self, path: &str, secret: Secret) -> Result<()> {
        validate_path(path)?;
        self.secrets.insert(path.to_owned(), secret);
        Ok(())
    }

    /// Removes the secret at `path`.
    ///
    /// # Errors
    /// Returns [`Error::SecretNotFound`] when `path` is not present.
    pub fn delete(&mut self, path: &str) -> Result<()> {
        if self.secrets.remove(path).is_none() {
            return Err(Error::SecretNotFound(path.to_owned()));
        }
        Ok(())
    }

    /// Returns a sorted list of secret paths that start with `prefix`.
    ///
    /// Pass an empty string to list all secrets.
    #[must_use]
    pub fn list(&self, prefix: &str) -> Vec<&str> {
        let mut keys: Vec<&str> = self
            .secrets
            .keys()
            .filter(|k| k.starts_with(prefix))
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        keys
    }

    /// Searches for secrets whose path or note contains `query` (case-insensitive).
    #[must_use]
    pub fn find(&self, query: &str) -> Vec<&str> {
        let q = query.to_lowercase();
        let mut keys: Vec<&str> = self
            .secrets
            .iter()
            .filter(|(k, v)| {
                k.to_lowercase().contains(&q) || v.note.to_lowercase().contains(&q)
            })
            .map(|(k, _)| k.as_str())
            .collect();
        keys.sort_unstable();
        keys
    }

    /// Renames a secret from `from` to `to`.
    ///
    /// # Errors
    /// - [`Error::SecretNotFound`] if `from` does not exist.
    /// - [`Error::SecretExists`] if `to` already exists.
    /// - [`Error::InvalidPath`] for an invalid `to` path.
    pub fn rename(&mut self, from: &str, to: &str) -> Result<()> {
        validate_path(to)?;
        if self.secrets.contains_key(to) {
            return Err(Error::SecretExists(to.to_owned()));
        }
        let secret = self
            .secrets
            .remove(from)
            .ok_or_else(|| Error::SecretNotFound(from.to_owned()))?;
        self.secrets.insert(to.to_owned(), secret);
        Ok(())
    }

    /// Re-encrypts the vault with a new passphrase.
    ///
    /// Replaces the in-memory passphrase; callers must still call [`Vault::save`].
    pub fn change_passphrase(&mut self, new_passphrase: Zeroizing<String>) {
        self.passphrase = new_passphrase;
    }

    /// Returns `true` if a secret exists at `path`.
    #[must_use]
    pub fn exists(&self, path: &str) -> bool {
        self.secrets.contains_key(path)
    }

    /// Returns the on-disk path of the vault file.
    #[must_use]
    pub fn vault_path(&self) -> &Path {
        &self.vault_path
    }

    /// Exports all secrets as a JSON string (plaintext — handle carefully).
    ///
    /// # Errors
    /// Returns [`Error::Json`] on serialization failure.
    pub fn export_json(&self) -> Result<String> {
        let data = VaultData {
            version: self.version,
            secrets: self
                .secrets
                .iter()
                .map(|(k, v)| (k.clone(), SecretData::from(v)))
                .collect(),
        };
        Ok(serde_json::to_string_pretty(&data)?)
    }

    /// Imports secrets from a JSON string previously produced by [`Vault::export_json`].
    ///
    /// Returns the number of secrets imported.
    ///
    /// # Errors
    /// Returns [`Error::Json`] on parse failure.
    pub fn import_json(&mut self, json: &str) -> Result<usize> {
        let data: VaultData = serde_json::from_str(json)?;
        let count = data.secrets.len();
        for (k, v) in data.secrets {
            self.secrets.insert(k, Secret::from(v));
        }
        Ok(count)
    }

    /// Imports secrets from a `.env`-formatted string (`KEY=VALUE` lines).
    ///
    /// Lines starting with `#` and blank lines are ignored.
    /// Keys are lowercased and `/` replaces `_` to form the secret path.
    ///
    /// Returns the number of secrets imported.
    pub fn import_dotenv(&mut self, content: &str) -> usize {
        let mut count = 0usize;
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                let path = key.trim().to_lowercase().replace('_', "/");
                let secret = Secret::new(value.trim());
                self.secrets.insert(path, secret);
                count += 1;
            }
        }
        count
    }
}

fn validate_path(path: &str) -> Result<()> {
    if path.is_empty() {
        return Err(Error::InvalidPath("path must not be empty".into()));
    }
    if path.starts_with('/') || path.ends_with('/') {
        return Err(Error::InvalidPath(
            "path must not start or end with '/'".into(),
        ));
    }
    if path.contains("//") {
        return Err(Error::InvalidPath(
            "path must not contain consecutive slashes".into(),
        ));
    }
    Ok(())
}

fn make_passphrase(s: &str) -> age::secrecy::SecretString {
    age::secrecy::SecretString::from(s.to_owned())
}

fn age_encrypt(plaintext: &[u8], passphrase: &str) -> Result<Vec<u8>> {
    let enc = age::Encryptor::with_user_passphrase(make_passphrase(passphrase));
    let mut output = Vec::new();
    let mut writer = enc
        .wrap_output(&mut output)
        .map_err(|e| Error::Encrypt(e.to_string()))?;
    writer
        .write_all(plaintext)
        .map_err(|e| Error::Encrypt(e.to_string()))?;
    writer
        .finish()
        .map_err(|e| Error::Encrypt(e.to_string()))?;
    Ok(output)
}

fn age_decrypt(ciphertext: &[u8], passphrase: &str) -> Result<Vec<u8>> {
    let dec = age::Decryptor::new_buffered(ciphertext)
        .map_err(|e| Error::Decrypt(e.to_string()))?;
    if !dec.is_scrypt() {
        return Err(Error::Decrypt(
            "vault was not encrypted with a passphrase".into(),
        ));
    }
    let identity = age::scrypt::Identity::new(make_passphrase(passphrase));
    let identity_ref: &dyn age::Identity = &identity;
    let mut reader = dec
        .decrypt(std::iter::once(identity_ref))
        .map_err(|_| Error::WrongPassphrase)?;
    let mut output = Vec::new();
    reader
        .read_to_end(&mut output)
        .map_err(|e| Error::Decrypt(e.to_string()))?;
    Ok(output)
}
