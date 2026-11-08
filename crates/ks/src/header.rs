//! Pre-decryption check of an entry's `age` header: the file must be a binary
//! (non-armored) age v1 file with exactly one `X25519` recipient stanza.
//! Other stanza tags (e.g. grease stanzas that `age` itself emits) are
//! tolerated, but `scrypt` is rejected outright so a planted passphrase file
//! can never reach passphrase logic. Armored text, malformed headers and
//! stanza floods are [`Error::Corrupt`].

use std::path::Path;

use crate::{Error, Result};

const MAGIC: &[u8] = b"age-encryption.org/v1";

/// Checks the header of `bytes` (a whole file already size-checked).
///
/// # Errors
///
/// [`Error::Corrupt`] for anything that is not a binary v1 file with exactly
/// one `X25519` stanza.
pub(crate) fn check_single_x25519(bytes: &[u8], path: &Path) -> Result<()> {
    let corrupt = || Error::Corrupt {
        path: path.to_path_buf(),
        reason: "not a single-recipient X25519 age file",
    };
    let rest = bytes.strip_prefix(MAGIC).ok_or_else(corrupt)?;
    let rest = rest.strip_prefix(b"\n").ok_or_else(corrupt)?;

    let mut stanzas = 0u32;
    let mut x25519 = 0u32;
    let mut x25519_body = 0u32;
    let mut in_x25519 = false;
    let mut empty_terminated = false;
    let mut footer_seen = false;
    for line in rest.split(|&b| b == b'\n') {
        if line.starts_with(b"---") {
            footer_seen = true;
            break;
        }
        if let Some(header) = line.strip_prefix(b"-> ") {
            stanzas += 1;
            if stanzas > 64 {
                return Err(corrupt());
            }
            let tag = header.split(|&b| b == b' ').next().unwrap_or_default();
            if tag == b"scrypt" {
                return Err(corrupt());
            }
            in_x25519 = tag == b"X25519";
            if in_x25519 {
                x25519 += 1;
            }
            empty_terminated = false;
        } else if line.is_empty() {
            // One empty line may terminate a stanza's body (age emits grease
            // stanzas with empty bodies and trailing empty lines).
            if stanzas == 0 || empty_terminated {
                return Err(corrupt());
            }
            empty_terminated = true;
        } else {
            // Stanza body lines are base64 and come after a stanza header.
            if stanzas == 0
                || empty_terminated
                || line.len() > 64
                || !line
                    .iter()
                    .all(|b| b.is_ascii_alphanumeric() || *b == b'+' || *b == b'/' || *b == b'=')
            {
                return Err(corrupt());
            }
            if in_x25519 {
                x25519_body += 1;
            }
        }
    }
    if !footer_seen || x25519 != 1 || x25519_body == 0 {
        return Err(corrupt());
    }
    Ok(())
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

    const P: &str = "x.age";

    fn check(bytes: &[u8]) -> Result<()> {
        check_single_x25519(bytes, Path::new(P))
    }

    #[test]
    fn grease_stanzas_are_tolerated() {
        check(b"age-encryption.org/v1\n-> X25519 arg\nYWJj\n-> CXP4-grease x\nYWJj\n--- m\n").unwrap();
    }

    #[test]
    fn single_x25519_ok() {
        // Real file produced by age to a random recipient.
        let id = age::x25519::Identity::generate();
        let recipient = id.to_public();
        let recipient: &dyn age::Recipient = &recipient;
        let encryptor = age::Encryptor::with_recipients(core::iter::once(recipient)).unwrap();
        let mut out = Vec::new();
        let mut w = encryptor.wrap_output(&mut out).unwrap();
        use std::io::Write as _;
        w.write_all(b"hi").unwrap();
        w.finish().unwrap();
        check(&out).unwrap();
    }

    #[test]
    fn rejects_everything_else() {
        let base = b"age-encryption.org/v1\n-> X25519 abcdef\nYWJj\n--- mac\npayload".to_vec();
        check(&base).unwrap();
        for bytes in [
            b"-----BEGIN AGE ENCRYPTED FILE-----\nage-encryption.org/v1\n".to_vec(),
            b"age-encryption.org/v1\n--- mac\n".to_vec(),
            b"age-encryption.org/v1\n-> scrypt abc\nYWJj\n--- m\n".to_vec(),
            b"age-encryption.org/v1\n-> X25519 a\nYWJj\n-> X25519 b\nYWJj\n--- m\n".to_vec(),
            b"age-encryption.org/v1\n-> X25519 a\nnot base64!\n--- m\n".to_vec(),
            b"age-encryption.org/v1\n-> X25519 a\n--- m\n".to_vec(),
            b"age-encryption.org/v1\n-> X25519 a\nYWJj".to_vec(),
            b"age-encryption.org/v1\n-> scrypt a\nYWJj\n-> X25519 b\nYWJj\n--- m\n".to_vec(),
            b"garbage".to_vec(),
            Vec::new(),
        ] {
            assert!(check(&bytes).is_err(), "{bytes:?}");
        }
    }
}
