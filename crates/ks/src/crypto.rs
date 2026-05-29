//! age-based cryptography: the building blocks for the whole store.
//!
//! This module bundles three tightly-coupled concerns that all operate on age
//! key material:
//!
//! - **Encryption primitives.** [`encrypt`] targets one or more X25519
//!   recipients and needs only public keys, so secrets can be written without
//!   ever unlocking the identity. [`decrypt`] needs the user's
//!   [`x25519::Identity`].
//! - **Identity file.** [`create_identity`] / [`load_identity`] manage an age
//!   scrypt (passphrase) container whose plaintext is the bech32 secret key.
//!   The format is interoperable with the `age` / `rage` CLIs
//!   (`age -d identity.age`).
//! - **Recipient list.** [`load_recipients`] / [`save_recipients`] read and
//!   write the plaintext `age1…` allow-list stored alongside the secrets.

use std::io::{Read as _, Write as _};
use std::path::Path;
use std::str::FromStr as _;

use age::secrecy::{ExposeSecret as _, SecretString};
use age::x25519;
use zeroize::Zeroizing;

use crate::error::{Error, Result};

/// Encrypts `plaintext` to one or more X25519 recipients (age recipient mode).
///
/// # Errors
/// Returns [`Error::Encrypt`] if `recipients` is empty or the age encoder fails.
pub fn encrypt(plaintext: &[u8], recipients: &[x25519::Recipient]) -> Result<Vec<u8>> {
    if recipients.is_empty() {
        return Err(Error::Encrypt("no recipients".into()));
    }
    let encryptor =
        age::Encryptor::with_recipients(recipients.iter().map(|r| -> &dyn age::Recipient { r }))
            .map_err(|e| Error::Encrypt(e.to_string()))?;

    let mut output = Vec::with_capacity(plaintext.len() + 256);
    let mut writer = encryptor
        .wrap_output(&mut output)
        .map_err(|e| Error::Encrypt(e.to_string()))?;
    writer
        .write_all(plaintext)
        .map_err(|e| Error::Encrypt(e.to_string()))?;
    writer.finish().map_err(|e| Error::Encrypt(e.to_string()))?;
    Ok(output)
}

/// Decrypts a recipient-mode `ciphertext` with the given X25519 identity.
///
/// The plaintext is returned in a [`Zeroizing`] buffer, scrubbed on drop.
///
/// # Errors
/// Returns [`Error::Decrypt`] if the file is passphrase-encrypted (wrong mode)
/// or the age decoder fails.
pub fn decrypt(ciphertext: &[u8], identity: &x25519::Identity) -> Result<Zeroizing<Vec<u8>>> {
    let decryptor =
        age::Decryptor::new_buffered(ciphertext).map_err(|e| Error::Decrypt(e.to_string()))?;
    if decryptor.is_scrypt() {
        return Err(Error::Decrypt(
            "file was encrypted with a passphrase, not a recipient".into(),
        ));
    }
    let identities: [&dyn age::Identity; 1] = [identity];
    let mut reader = decryptor
        .decrypt(identities.into_iter())
        .map_err(|e| Error::Decrypt(e.to_string()))?;

    let mut buf = Zeroizing::new(Vec::with_capacity(ciphertext.len()));
    reader
        .read_to_end(&mut buf)
        .map_err(|e| Error::Decrypt(e.to_string()))?;
    Ok(buf)
}

/// Generates a new X25519 identity, encrypts it with `passphrase`, and writes
/// it to `path` (mode `0o600` on Unix). Refuses to overwrite an existing file.
///
/// # Errors
/// - [`Error::IdentityExists`] if `path` already exists.
/// - [`Error::Io`] / [`Error::Encrypt`] on filesystem or age failures.
pub fn create_identity(path: &Path, passphrase: SecretString) -> Result<x25519::Identity> {
    if path.exists() {
        return Err(Error::IdentityExists(path.to_path_buf()));
    }
    let identity = x25519::Identity::generate();
    let serialised = identity.to_string();
    let ciphertext = encrypt_with_passphrase(serialised.expose_secret().as_bytes(), passphrase)?;
    write_atomic(path, &ciphertext)?;
    Ok(identity)
}

/// Loads and decrypts an identity file with the supplied passphrase.
///
/// # Errors
/// - [`Error::IdentityNotFound`] if the file is absent.
/// - [`Error::WrongPassphrase`] if `passphrase` does not match.
/// - [`Error::Decrypt`] / [`Error::Io`] on other failures.
pub fn load_identity(path: &Path, passphrase: SecretString) -> Result<x25519::Identity> {
    if !path.exists() {
        return Err(Error::IdentityNotFound(path.to_path_buf()));
    }
    let ciphertext = std::fs::read(path)?;
    let plaintext = decrypt_with_passphrase(&ciphertext, passphrase)?;
    parse_identity(&plaintext)
}

/// Re-encrypts an existing identity file with a new passphrase.
///
/// # Errors
/// Same as [`load_identity`] plus any encryption errors.
pub fn change_passphrase(path: &Path, current: SecretString, new: SecretString) -> Result<()> {
    let identity = load_identity(path, current)?;
    let serialised = identity.to_string();
    let ciphertext = encrypt_with_passphrase(serialised.expose_secret().as_bytes(), new)?;
    write_atomic(path, &ciphertext)?;
    Ok(())
}

/// Parses a recipients-file body into public keys, preserving order and
/// stripping `#` comments and blank lines.
///
/// # Errors
/// Returns [`Error::InvalidRecipient`] if a non-comment line fails to parse.
pub fn parse_recipients(text: &str) -> Result<Vec<x25519::Recipient>> {
    let mut out = Vec::new();
    for (idx, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let recipient = x25519::Recipient::from_str(line)
            .map_err(|e| Error::InvalidRecipient(format!("line {}: {e}", idx.saturating_add(1))))?;
        out.push(recipient);
    }
    Ok(out)
}

/// Reads and parses the recipients file at `path`.
///
/// # Errors
/// - [`Error::NoRecipients`] if the file is missing or contains no keys.
/// - [`Error::Io`] / [`Error::InvalidRecipient`] on read or parse failures.
pub fn load_recipients(path: &Path) -> Result<Vec<x25519::Recipient>> {
    if !path.exists() {
        return Err(Error::NoRecipients(path.to_path_buf()));
    }
    let recipients = parse_recipients(&std::fs::read_to_string(path)?)?;
    if recipients.is_empty() {
        return Err(Error::NoRecipients(path.to_path_buf()));
    }
    Ok(recipients)
}

/// Atomically writes the recipients file at `path` with a header comment.
///
/// # Errors
/// Returns [`Error::Io`] on any filesystem failure.
pub fn save_recipients(path: &Path, recipients: &[x25519::Recipient]) -> Result<()> {
    let mut body = String::from(
        "# ks recipients — public keys allowed to decrypt this store.\n\
         # Add one with `ks recipients add <age1...>`.\n",
    );
    for r in recipients {
        body.push_str(&r.to_string());
        body.push('\n');
    }
    write_atomic(path, body.as_bytes())
}

/// Returns `true` if `target` is present in `list` (by textual public-key form).
#[must_use]
pub fn recipients_contain(list: &[x25519::Recipient], target: &x25519::Recipient) -> bool {
    let needle = target.to_string();
    list.iter().any(|r| r.to_string() == needle)
}

/// Atomically writes `bytes` to `path` (mode `0o600` on Unix): write a sibling
/// temp file, fsync, set permissions, then rename over the target.
///
/// # Errors
/// Returns [`Error::Io`] on any filesystem failure.
pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("ks-tmp");
    {
        let mut file = std::fs::File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    set_owner_only(&tmp)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

fn encrypt_with_passphrase(plaintext: &[u8], passphrase: SecretString) -> Result<Vec<u8>> {
    let encryptor = age::Encryptor::with_user_passphrase(passphrase);
    let mut output = Vec::with_capacity(plaintext.len() + 256);
    let mut writer = encryptor
        .wrap_output(&mut output)
        .map_err(|e| Error::Encrypt(e.to_string()))?;
    writer
        .write_all(plaintext)
        .map_err(|e| Error::Encrypt(e.to_string()))?;
    writer.finish().map_err(|e| Error::Encrypt(e.to_string()))?;
    Ok(output)
}

fn decrypt_with_passphrase(
    ciphertext: &[u8],
    passphrase: SecretString,
) -> Result<Zeroizing<Vec<u8>>> {
    let decryptor =
        age::Decryptor::new_buffered(ciphertext).map_err(|e| Error::Decrypt(e.to_string()))?;
    if !decryptor.is_scrypt() {
        return Err(Error::Decrypt(
            "file was encrypted to a recipient, not a passphrase".into(),
        ));
    }
    let identity = age::scrypt::Identity::new(passphrase);
    let identities: [&dyn age::Identity; 1] = [&identity];
    let mut reader = decryptor
        .decrypt(identities.into_iter())
        .map_err(|_| Error::WrongPassphrase)?;

    let mut buf = Zeroizing::new(Vec::with_capacity(ciphertext.len()));
    reader
        .read_to_end(&mut buf)
        .map_err(|e| Error::Decrypt(e.to_string()))?;
    Ok(buf)
}

/// Extracts an [`x25519::Identity`] from a decrypted identity payload.
///
/// Accepts bare (`AGE-SECRET-KEY-1…`) and age-keygen formatted input; the
/// first non-comment, non-empty line is treated as the secret key.
fn parse_identity(plaintext: &[u8]) -> Result<x25519::Identity> {
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
        let dir = std::env::temp_dir().join(format!("ks-crypto-{}", rand::random::<u64>()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn recipient_roundtrip() {
        let identity = x25519::Identity::generate();
        let ct = encrypt(b"super secret api token", &[identity.to_public()]).expect("encrypt");
        let pt = decrypt(&ct, &identity).expect("decrypt");
        assert_eq!(&pt[..], b"super secret api token");
    }

    #[test]
    fn identity_create_load_roundtrip() {
        let path = tempdir().join("identity.age");
        let pp = SecretString::from("hunter2".to_owned());
        let created = create_identity(&path, pp.clone()).expect("create");
        let loaded = load_identity(&path, pp).expect("load");
        assert_eq!(
            created.to_public().to_string(),
            loaded.to_public().to_string()
        );
    }

    #[test]
    fn identity_refuses_overwrite() {
        let path = tempdir().join("identity.age");
        let pp = SecretString::from("pw".to_owned());
        create_identity(&path, pp.clone()).expect("first");
        assert!(matches!(
            create_identity(&path, pp),
            Err(Error::IdentityExists(_))
        ));
    }

    #[test]
    fn identity_wrong_passphrase_distinguishable() {
        let path = tempdir().join("identity.age");
        create_identity(&path, SecretString::from("right".to_owned())).expect("create");
        let err = load_identity(&path, SecretString::from("wrong".to_owned()))
            .err()
            .expect("must fail");
        assert!(matches!(err, Error::WrongPassphrase));
    }

    #[test]
    fn change_passphrase_works() {
        let path = tempdir().join("identity.age");
        let one = SecretString::from("one".to_owned());
        let two = SecretString::from("two".to_owned());
        create_identity(&path, one.clone()).expect("create");
        change_passphrase(&path, one.clone(), two.clone()).expect("change");
        assert!(load_identity(&path, one).is_err());
        assert!(load_identity(&path, two).is_ok());
    }

    #[test]
    fn recipients_parse_skips_comments() {
        let id = x25519::Identity::generate();
        let pubkey = id.to_public().to_string();
        let parsed = parse_recipients(&format!("# c\n\n{pubkey}\n")).expect("parse");
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed.first().expect("one recipient").to_string(), pubkey);
    }

    #[test]
    fn recipients_save_load_roundtrip() {
        let path = tempdir().join(".age-recipients");
        let id = x25519::Identity::generate();
        let r = id.to_public();
        save_recipients(&path, std::slice::from_ref(&r)).expect("save");
        let loaded = load_recipients(&path).expect("load");
        assert_eq!(loaded.len(), 1);
        assert!(recipients_contain(&loaded, &r));
    }

    #[test]
    fn recipients_reject_invalid() {
        assert!(parse_recipients("not-a-key").is_err());
    }
}
