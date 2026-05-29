//! Plaintext secret model.
//!
//! Following the `pass` / `gopass` convention, a secret is just UTF-8 text:
//!
//! ```text
//! correct-horse-battery-staple
//! user: alice
//! url: https://github.com
//! otpauth://totp/GitHub:alice?secret=JBSW...
//!
//! free-form notes go here
//! ```
//!
//! - The **first line** is the primary value (password / token).
//! - Any subsequent `key: value` line is a queryable **field**.
//! - Everything else is free-form body text.
//!
//! Because the on-disk form is exactly this text (encrypted with age and
//! nothing else), `age -d secret.age` yields human-readable output that
//! interoperates with the upstream `age` / `rage` CLIs. The full text is held
//! in a [`Zeroizing`] buffer and scrubbed from memory on drop.

use std::fmt;

use zeroize::Zeroizing;

/// An in-memory secret: the decrypted plaintext of one `.age` file.
#[derive(Clone)]
pub struct Secret {
    raw: Zeroizing<String>,
}

impl Secret {
    /// Creates a secret from raw plaintext (first line is the primary value).
    #[must_use]
    pub fn new(raw: impl Into<String>) -> Self {
        Self {
            raw: Zeroizing::new(raw.into()),
        }
    }

    /// Returns the primary value: the first line, without the trailing newline.
    #[must_use]
    pub fn password(&self) -> &str {
        self.raw
            .split('\n')
            .next()
            .unwrap_or("")
            .trim_end_matches('\r')
    }

    /// Returns the value of the first `key: value` field matching `key`.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&str> {
        self.fields().find(|(k, _)| *k == key).map(|(_, v)| v)
    }

    /// Iterates over all `key: value` fields (after the first line), in order.
    pub fn fields(&self) -> impl Iterator<Item = (&str, &str)> {
        self.raw.lines().skip(1).filter_map(parse_field)
    }

    /// Returns the field keys, in document order.
    #[must_use]
    pub fn keys(&self) -> Vec<&str> {
        self.fields().map(|(k, _)| k).collect()
    }

    /// Returns a TOTP source for `ks otp`: an explicit `otpauth`/`otp`/`totp`
    /// field if present, otherwise the primary value, otherwise `None`.
    #[must_use]
    pub fn otp(&self) -> Option<&str> {
        for (k, v) in self.fields() {
            if matches!(k, "otpauth" | "otp" | "totp") {
                return Some(v.trim());
            }
        }
        let pw = self.password();
        (!pw.is_empty()).then_some(pw)
    }

    /// Returns the full decrypted plaintext.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.raw
    }

    /// Returns the plaintext bytes for encryption.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        self.raw.as_bytes()
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Secret")
            .field("raw", &"<redacted>")
            .finish()
    }
}

/// Parses a `key: value` field line. The key must be non-empty and contain no
/// whitespace, which avoids misreading prose like `Note: see below` as a field.
fn parse_field(line: &str) -> Option<(&str, &str)> {
    let (key, value) = line.split_once(": ")?;
    if key.is_empty() || key.contains(char::is_whitespace) {
        return None;
    }
    Some((key, value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_is_first_line() {
        let s = Secret::new("hunter2\nuser: alice\n");
        assert_eq!(s.password(), "hunter2");
    }

    #[test]
    fn password_strips_carriage_return() {
        let s = Secret::new("hunter2\r\nuser: alice");
        assert_eq!(s.password(), "hunter2");
    }

    #[test]
    fn fields_are_parsed_and_queryable() {
        let s = Secret::new("pw\nuser: alice\nurl: https://x.test\n");
        assert_eq!(s.get("user"), Some("alice"));
        assert_eq!(s.get("url"), Some("https://x.test"));
        assert_eq!(s.get("missing"), None);
        assert_eq!(s.keys(), vec!["user", "url"]);
    }

    #[test]
    fn prose_with_colon_is_not_a_field() {
        let s = Secret::new("pw\nNote that: this is prose\n");
        assert!(s.keys().is_empty());
    }

    #[test]
    fn first_line_is_never_a_field() {
        let s = Secret::new("user: alice\n");
        assert!(s.keys().is_empty());
        assert_eq!(s.password(), "user: alice");
    }

    #[test]
    fn otp_prefers_explicit_field() {
        let s = Secret::new("pw\notpauth: otpauth://totp/x?secret=ABC\n");
        assert_eq!(s.otp(), Some("otpauth://totp/x?secret=ABC"));
    }

    #[test]
    fn otp_falls_back_to_password() {
        let s = Secret::new("JBSWY3DPEHPK3PXP\n");
        assert_eq!(s.otp(), Some("JBSWY3DPEHPK3PXP"));
    }

    #[test]
    fn debug_redacts_contents() {
        let s = Secret::new("topsecret\n");
        assert!(!format!("{s:?}").contains("topsecret"));
    }
}
