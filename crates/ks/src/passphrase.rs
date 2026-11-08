//! Master passphrase: policy checks and the passphrase-sealed identity file.

use std::io::{Read as _, Write as _};

use age::secrecy::{ExposeSecret, SecretString};

use crate::{Error, LocalKey, Result};

const WORK_FACTOR: u8 = 18;
const MAX_WORK_FACTOR: u8 = 20;
const MIN_LENGTH: usize = 15;

/// Encrypts the identity file text under the passphrase with `age` scrypt
/// (work factor 2^18, binary output).
///
/// # Errors
///
/// [`Error::Internal`]-flavoured I/O errors if encryption fails.
pub(crate) fn seal_identity(key: &LocalKey, passphrase: &SecretString) -> Result<Vec<u8>> {
    let mut recipient = age::scrypt::Recipient::new(passphrase.clone());
    recipient.set_work_factor(WORK_FACTOR);
    let recipient: &dyn age::Recipient = &recipient;
    let encryptor = age::Encryptor::with_recipients(core::iter::once(recipient))
        .map_err(|_| Error::Internal("age encryption setup failed"))?;
    let plaintext = key.to_identity_text();
    let mut out = Vec::new();
    let mut writer = encryptor.wrap_output(&mut out).map_err(|e| Error::Io {
        path: "identity file".into(),
        source: e,
    })?;
    writer
        .write_all(plaintext.expose_secret().as_bytes())
        .and_then(|()| writer.finish().map(|_| ()))
        .map_err(|e| Error::Io {
            path: "identity file".into(),
            source: e,
        })?;
    drop(plaintext);
    Ok(out)
}

/// Decrypts an `age` scrypt identity file and parses the contained key(s).
///
/// # Errors
///
/// - [`Error::Corrupt`] for non-scrypt files, work factors above 2^20, or
///   malformed plaintext.
/// - [`Error::WrongPassphrase`] when decryption fails.
pub(crate) fn open_identity(bytes: &[u8], passphrase: &SecretString) -> Result<LocalKey> {
    let corrupt = |reason: &'static str| Error::Corrupt {
        path: ".identity.age".into(),
        reason,
    };
    let decryptor = age::Decryptor::new_buffered(bytes).map_err(|_| corrupt("not an age file"))?;
    if !decryptor.is_scrypt() {
        return Err(corrupt("not passphrase-encrypted"));
    }
    let mut scrypt_identity = age::scrypt::Identity::new(passphrase.clone());
    scrypt_identity.set_max_work_factor(MAX_WORK_FACTOR);
    let identity: &dyn age::Identity = &scrypt_identity;
    let reader = decryptor
        .decrypt(core::iter::once(identity))
        .map_err(|e| match e {
            age::DecryptError::ExcessiveWork { .. } => corrupt("work factor above 2^20"),
            age::DecryptError::DecryptionFailed | age::DecryptError::NoMatchingKeys => {
                Error::WrongPassphrase
            }
            _ => corrupt("cannot be decrypted"),
        })?;
    let mut plaintext = zeroize::Zeroizing::new(Vec::with_capacity(bytes.len()));
    reader
        .take(1024 * 1024)
        .read_to_end(&mut plaintext)
        .map_err(|_| corrupt("decrypted plaintext is not text"))?;
    core::str::from_utf8(&plaintext).map_err(|_| corrupt("decrypted plaintext is not UTF-8"))?;
    let text = String::from_utf8(core::mem::take(&mut *plaintext))
        .map_err(|_| corrupt("decrypted plaintext is not UTF-8"))?;
    LocalKey::from_identity_text(&SecretString::from(text))
}

/// Validates a new passphrase. Returns warnings (non-fatal concerns).
///
/// # Errors
///
/// [`Error::WeakPassphrase`] when shorter than 15 Unicode scalar values,
/// containing control characters, or scoring below `zxcvbn` 3.
pub fn check_new_passphrase(passphrase: &SecretString) -> Result<Vec<String>> {
    let text = passphrase.expose_secret();
    if text.chars().count() < MIN_LENGTH {
        return Err(Error::WeakPassphrase(
            "must be at least 15 characters".into(),
        ));
    }
    if text.chars().any(char::is_control) {
        return Err(Error::WeakPassphrase(
            "must not contain control characters".into(),
        ));
    }
    let entropy = zxcvbn::zxcvbn(text, &[]);
    if entropy.score() < zxcvbn::Score::Three {
        let mut msg = String::from("zxcvbn score below 3");
        if let Some(feedback) = entropy.feedback() {
            if let Some(warning) = feedback.warning() {
                msg.push_str(": ");
                msg.push_str(&warning.to_string());
            }
            for suggestion in feedback.suggestions() {
                msg.push_str("; ");
                msg.push_str(&suggestion.to_string());
            }
        }
        return Err(Error::WeakPassphrase(msg));
    }
    let mut warnings = Vec::new();
    if !text.is_ascii() {
        warnings.push(
            "passphrase contains non-ASCII characters; different devices or input methods may produce different bytes"
                .to_owned(),
        );
    }
    Ok(warnings)
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::panic,
        clippy::missing_assert_message,
        clippy::let_underscore_must_use,
        clippy::items_after_statements,
        clippy::shadow_unrelated,
        clippy::indexing_slicing,
        clippy::needless_collect,
        reason = "unit tests may panic on fixture setup and use terse assertions"
    )]
    use super::*;
    use age::secrecy::SecretString;
    use std::sync::LazyLock;

    const PASS: &str = "a correct horse battery staple fixture";

    static SEALED: LazyLock<(LocalKey, Vec<u8>)> = LazyLock::new(|| {
        let key = LocalKey::generate();
        let sealed = seal_identity(&key, &SecretString::from(PASS)).unwrap();
        (key, sealed)
    });

    #[test]
    fn policy_boundaries() {
        let pass = |s: &str| check_new_passphrase(&SecretString::from(s));
        assert!(pass("fourteen-char!").is_err()); // 14
        assert!(pass("aaaaaaaaaaaaaaa").is_err()); // 15 chars but weak
        assert!(pass("a very long and memorable passphrase 42!").is_ok());
        assert!(pass("contains\u{7}control-characters-in-here").is_err());
        let warnings = pass("this passphrase hás unicode inside it").unwrap();
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn seal_and_open() {
        let (_, sealed) = &*SEALED;
        let key = open_identity(sealed, &SecretString::from(PASS)).unwrap();
        assert_eq!(key.to_identity_text().expose_secret().lines().count(), 2);
        match open_identity(sealed, &SecretString::from("wrong passphrase entirely")) {
            Err(Error::WrongPassphrase) => {}
            other => panic!("expected WrongPassphrase, got {other:?}"),
        }
        match open_identity(b"not an age file", &SecretString::from(PASS)) {
            Err(Error::Corrupt { .. }) => {}
            other => panic!("expected Corrupt, got {other:?}"),
        }
    }

    #[test]
    fn excessive_work_is_corrupt_fast() {
        // Craft an identity file at work factor 2^21 (> cap 20).
        let key = LocalKey::generate();
        let mut recipient = age::scrypt::Recipient::new(SecretString::from(PASS));
        recipient.set_work_factor(21);
        let recipient: &dyn age::Recipient = &recipient;
        let encryptor = age::Encryptor::with_recipients(core::iter::once(recipient)).unwrap();
        let mut sealed = Vec::new();
        let mut w = encryptor.wrap_output(&mut sealed).unwrap();
        use std::io::Write as _;
        w.write_all(key.to_identity_text().expose_secret().as_bytes())
            .unwrap();
        w.finish().unwrap();

        let start = std::time::Instant::now();
        match open_identity(&sealed, &SecretString::from(PASS)) {
            Err(Error::Corrupt { .. }) => {}
            other => panic!("expected Corrupt, got {other:?}"),
        }
        assert!(
            start.elapsed().as_secs() < 1,
            "work-factor rejection must be fast"
        );
    }
}
