//! Age identity file management.
//!
//! The identity file is an age scrypt-mode (passphrase-encrypted) container
//! whose plaintext is the bech32-encoded X25519 secret key (`AGE-SECRET-KEY-1…`).
//!
//! This is the same format produced by `age -p -o identity.age identity.txt`
//! and is fully interoperable with the upstream `age` and `rage` CLIs.

use std::io::Write as _;
use std::path::Path;
use std::str::FromStr as _;

use age::secrecy::{ExposeSecret as _, SecretString};
use age::x25519;
use zeroize::Zeroizing;

use crate::crypto;
use crate::error::{Error, Result};

/// Generates a new X25519 identity, encrypts it with `passphrase`, and writes
/// the result to `path`. Refuses to overwrite an existing file.
///
/// # Errors
/// - [`Error::IdentityExists`] if `path` already exists.
/// - [`Error::Io`] / [`Error::Encrypt`] on filesystem or age failures.
pub fn create(path: &Path, passphrase: SecretString) -> Result<x25519::Identity> {
    if path.exists() {
        return Err(Error::IdentityExists(path.to_path_buf()));
    }
    let identity = x25519::Identity::generate();
    let serialised = identity.to_string();
    let ciphertext =
        crypto::encrypt_with_passphrase(serialised.expose_secret().as_bytes(), passphrase)?;
    write_atomic(path, &ciphertext)?;
    Ok(identity)
}

/// Loads and decrypts an identity file with the supplied passphrase.
///
/// # Errors
/// - [`Error::IdentityNotFound`] if the file is absent.
/// - [`Error::WrongPassphrase`] if `passphrase` does not match.
/// - [`Error::Decrypt`] / [`Error::Io`] on other failures.
pub fn load(path: &Path, passphrase: SecretString) -> Result<x25519::Identity> {
    if !path.exists() {
        return Err(Error::IdentityNotFound(path.to_path_buf()));
    }
    let ciphertext = std::fs::read(path)?;
    let plaintext = crypto::decrypt_with_passphrase(&ciphertext, passphrase)?;
    parse_secret(&plaintext)
}

/// Re-encrypts an existing identity file with a new passphrase.
///
/// # Errors
/// Same as [`load`] plus any encryption errors.
pub fn change_passphrase(path: &Path, current: SecretString, new: SecretString) -> Result<()> {
    let identity = load(path, current)?;
    let serialised = identity.to_string();
    let ciphertext = crypto::encrypt_with_passphrase(serialised.expose_secret().as_bytes(), new)?;
    write_atomic(path, &ciphertext)?;
    Ok(())
}

/// Parses a decrypted identity payload into an [`x25519::Identity`].
///
/// Accepts both bare (`AGE-SECRET-KEY-1…`) and age-keygen formatted input
/// (with comment header lines). The first non-comment, non-empty line is
/// expected to be the secret key.
fn parse_secret(plaintext: &[u8]) -> Result<x25519::Identity> {
    let text = std::str::from_utf8(plaintext)
        .map_err(|e| Error::Decrypt(format!("identity is not valid UTF-8: {e}")))?;
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        return x25519::Identity::from_str(line)
            .map_err(|e| Error::Decrypt(format!("invalid identity payload: {e}")));
    }
    Err(Error::Decrypt("identity file is empty".into()))
}

/// Parses a raw secret key string into an [`x25519::Identity`].
///
/// # Errors
/// Returns [`Error::Decrypt`] if the string is not a valid age identity.
pub fn parse(secret_key: &str) -> Result<x25519::Identity> {
    x25519::Identity::from_str(secret_key.trim())
        .map_err(|e| Error::Decrypt(format!("invalid identity key: {e}")))
}

/// Serialises an identity back to its bech32 secret-key string (zeroized on drop).
#[must_use]
pub fn to_secret_string(identity: &x25519::Identity) -> Zeroizing<String> {
    Zeroizing::new(identity.to_string().expose_secret().to_owned())
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

#[cfg(test)]
mod tests {
    use super::*;

    fn tempdir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("ks-id-{}", rand::random::<u64>()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn create_then_load_roundtrip() {
        let dir = tempdir();
        let path = dir.join("identity.age");
        let pp = SecretString::from("hunter2".to_owned());
        let created = create(&path, pp.clone()).expect("create");
        let loaded = load(&path, pp).expect("load");
        assert_eq!(
            to_secret_string(&created).as_str(),
            to_secret_string(&loaded).as_str(),
        );
    }

    #[test]
    fn refuses_overwrite() {
        let dir = tempdir();
        let path = dir.join("identity.age");
        let pp = SecretString::from("pw".to_owned());
        create(&path, pp.clone()).expect("first create");
        let err = create(&path, pp).err().expect("second create should fail");
        assert!(matches!(err, Error::IdentityExists(_)));
    }

    #[test]
    fn wrong_passphrase_distinguishable() {
        let dir = tempdir();
        let path = dir.join("identity.age");
        create(&path, SecretString::from("right".to_owned())).expect("create");
        let err = load(&path, SecretString::from("wrong".to_owned()))
            .err()
            .expect("must fail");
        assert!(matches!(err, Error::WrongPassphrase));
    }

    #[test]
    fn change_passphrase_works() {
        let dir = tempdir();
        let path = dir.join("identity.age");
        let pp1 = SecretString::from("one".to_owned());
        let pp2 = SecretString::from("two".to_owned());
        create(&path, pp1.clone()).expect("create");
        change_passphrase(&path, pp1.clone(), pp2.clone()).expect("change");
        assert!(load(&path, pp1).is_err());
        assert!(load(&path, pp2).is_ok());
    }
}
