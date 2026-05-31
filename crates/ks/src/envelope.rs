//! Versioned secret envelope: binds the logical path into the encrypted payload.
//!
//! Before encryption, a secret's bytes are wrapped with a small header that
//! records the format version, the secret kind (text/binary) and the logical
//! path the secret lives at. After decryption the header is verified: if the
//! bound path does not match where the file actually sits, [`unwrap`] fails with
//! [`Error::Tampered`]. This detects an attacker (or a botched sync) relocating,
//! swapping or rolling back ciphertext files — something plain age files cannot
//! detect on their own, since an age file carries no notion of its own name.
//!
//! The header is plain text and the payload follows verbatim to EOF, so a text
//! secret still reads cleanly through `age -d secret.age`:
//!
//! ```text
//! ksenv/1
//! text
//! github/token
//!
//! <payload bytes…>
//! ```

use zeroize::Zeroizing;

use crate::error::{Error, Result};
use crate::secret::SecretKind;

/// Magic + format version, terminated by a newline. Bump the version when the
/// header layout changes.
const MAGIC: &[u8] = b"ksenv/1\n";

const TAG_TEXT: &[u8] = b"text";
const TAG_BINARY: &[u8] = b"binary";

/// Wraps `payload` in a versioned envelope binding it to `logical` and `kind`.
///
/// The result is held in a [`Zeroizing`] buffer so the assembled plaintext is
/// scrubbed once it has been handed to the encryptor.
#[must_use]
pub(crate) fn wrap(logical: &str, kind: SecretKind, payload: &[u8]) -> Zeroizing<Vec<u8>> {
    let tag = match kind {
        SecretKind::Text => TAG_TEXT,
        SecretKind::Binary => TAG_BINARY,
    };
    let mut out = Zeroizing::new(Vec::with_capacity(
        MAGIC.len() + tag.len() + logical.len() + payload.len() + 3,
    ));
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(tag);
    out.push(b'\n');
    out.extend_from_slice(logical.as_bytes());
    out.push(b'\n');
    out.push(b'\n');
    out.extend_from_slice(payload);
    out
}

/// Unwraps an envelope, verifying the magic/version and that the bound path
/// matches `expected`. Returns the secret kind and the raw payload.
///
/// # Errors
/// Returns [`Error::Tampered`] if the header is missing or unsupported (a legacy
/// or corrupt file) or the bound path does not match `expected` (a relocated,
/// swapped, or otherwise misfiled secret).
pub(crate) fn unwrap(expected: &str, plaintext: &[u8]) -> Result<(SecretKind, Vec<u8>)> {
    let tampered = |reason: String| Error::Tampered {
        path: expected.to_owned(),
        reason,
    };

    let rest = plaintext.strip_prefix(MAGIC).ok_or_else(|| {
        tampered("missing or unsupported envelope header (legacy or corrupt secret)".to_owned())
    })?;

    let (tag, rest) =
        split_line(rest).ok_or_else(|| tampered("truncated envelope header".to_owned()))?;
    let kind = match tag {
        TAG_TEXT => SecretKind::Text,
        TAG_BINARY => SecretKind::Binary,
        _ => return Err(tampered("unknown secret kind in envelope".to_owned())),
    };

    let (path_line, rest) =
        split_line(rest).ok_or_else(|| tampered("truncated envelope header".to_owned()))?;
    let payload = rest
        .strip_prefix(b"\n")
        .ok_or_else(|| tampered("malformed envelope (missing header terminator)".to_owned()))?;

    if path_line != expected.as_bytes() {
        return Err(tampered(format!(
            "bound path `{}` does not match its location",
            String::from_utf8_lossy(path_line)
        )));
    }

    Ok((kind, payload.to_vec()))
}

/// Splits off the bytes up to the next `\n`, returning `(line, remainder)` with
/// the newline consumed. `None` if there is no newline.
fn split_line(bytes: &[u8]) -> Option<(&[u8], &[u8])> {
    let idx = bytes.iter().position(|&c| c == b'\n')?;
    let line = bytes.get(..idx)?;
    let rest = bytes.get(idx.saturating_add(1)..)?;
    Some((line, rest))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_roundtrip() {
        let wrapped = wrap("github/token", SecretKind::Text, b"ghp_xxx\nuser: alice\n");
        let (kind, payload) = unwrap("github/token", &wrapped).expect("unwrap");
        assert_eq!(kind, SecretKind::Text);
        assert_eq!(&payload, b"ghp_xxx\nuser: alice\n");
    }

    #[test]
    fn binary_roundtrip_with_arbitrary_bytes() {
        let raw = vec![0u8, b'\n', 0xff, b'k', b's', 0x00, 0x0a];
        let wrapped = wrap("certs/key.p12", SecretKind::Binary, &raw);
        let (kind, payload) = unwrap("certs/key.p12", &wrapped).expect("unwrap");
        assert_eq!(kind, SecretKind::Binary);
        assert_eq!(payload, raw);
    }

    #[test]
    fn empty_payload_roundtrip() {
        let wrapped = wrap("a/b", SecretKind::Text, b"");
        let (kind, payload) = unwrap("a/b", &wrapped).expect("unwrap");
        assert_eq!(kind, SecretKind::Text);
        assert!(payload.is_empty());
    }

    #[test]
    fn wrong_bound_path_is_tampered() {
        let wrapped = wrap("a", SecretKind::Text, b"secret-a");
        let err = unwrap("b", &wrapped).expect_err("must reject");
        assert!(matches!(err, Error::Tampered { .. }));
    }

    #[test]
    fn legacy_or_corrupt_payload_is_tampered() {
        // A bare age plaintext with no envelope header (pre-envelope format).
        let err = unwrap("a", b"just a raw secret\n").expect_err("must reject");
        assert!(matches!(err, Error::Tampered { .. }));
    }

    #[test]
    fn payload_may_contain_header_like_lines() {
        // A payload whose own bytes look like a header must not confuse parsing.
        let body = b"ksenv/1\ntext\nelsewhere\n\nreal payload";
        let wrapped = wrap("real/path", SecretKind::Binary, body);
        let (kind, payload) = unwrap("real/path", &wrapped).expect("unwrap");
        assert_eq!(kind, SecretKind::Binary);
        assert_eq!(&payload, body);
    }
}
