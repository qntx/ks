//! Thin, allocation-conscious wrappers around the `age` crate.
//!
//! All inputs/outputs are byte buffers. Plaintext outputs are returned in a
//! [`Zeroizing`] buffer so they are scrubbed on drop.

use std::io::{Read as _, Write as _};

use age::secrecy::SecretString;
use age::x25519;
use zeroize::Zeroizing;

use crate::error::{Error, Result};

/// Encrypts `plaintext` to one or more X25519 recipients (age recipient mode).
///
/// # Errors
/// Returns [`Error::Encrypt`] if `recipients` is empty or the underlying
/// `age` encoder fails.
pub fn encrypt_to_recipients(
    plaintext: &[u8],
    recipients: &[x25519::Recipient],
) -> Result<Vec<u8>> {
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
/// # Errors
/// Returns [`Error::Decrypt`] for any decoder error other than a missing match,
/// which is mapped to [`Error::WrongPassphrase`]-style semantics by the caller.
pub fn decrypt_with_identity(
    ciphertext: &[u8],
    identity: &x25519::Identity,
) -> Result<Zeroizing<Vec<u8>>> {
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

/// Encrypts `plaintext` with a user passphrase (age scrypt mode).
///
/// # Errors
/// Returns [`Error::Encrypt`] on any underlying `age` error.
pub fn encrypt_with_passphrase(plaintext: &[u8], passphrase: SecretString) -> Result<Vec<u8>> {
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

/// Decrypts a passphrase-mode `ciphertext`.
///
/// # Errors
/// Returns [`Error::WrongPassphrase`] when the file is scrypt-encrypted but
/// the passphrase does not match.
/// Returns [`Error::Decrypt`] for any other decoder error or if the file is
/// not in passphrase mode.
pub fn decrypt_with_passphrase(
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passphrase_roundtrip() {
        let plaintext = b"hello, ks!";
        let pp = SecretString::from("hunter2".to_owned());
        let ct = encrypt_with_passphrase(plaintext, pp.clone()).expect("encrypt");
        let pt = decrypt_with_passphrase(&ct, pp).expect("decrypt");
        assert_eq!(&pt[..], plaintext);
    }

    #[test]
    fn passphrase_wrong_passphrase_distinguishable() {
        let pp = SecretString::from("right".to_owned());
        let ct = encrypt_with_passphrase(b"data", pp).expect("encrypt");
        let bad = SecretString::from("wrong".to_owned());
        let err = decrypt_with_passphrase(&ct, bad).expect_err("must fail");
        assert!(matches!(err, Error::WrongPassphrase), "got {err:?}");
    }

    #[test]
    fn recipient_roundtrip() {
        let identity = x25519::Identity::generate();
        let recipient = identity.to_public();
        let plaintext = b"super secret api token";
        let ct = encrypt_to_recipients(plaintext, &[recipient]).expect("encrypt");
        let pt = decrypt_with_identity(&ct, &identity).expect("decrypt");
        assert_eq!(&pt[..], plaintext);
    }
}
