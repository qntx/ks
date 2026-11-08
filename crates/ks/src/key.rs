//! The vault key `K` and its recipient `R` (in memory only).

use std::collections::HashSet;
use std::fmt;
use std::str::FromStr;

use age::secrecy::{ExposeSecret, SecretString};
use age_core::format::{FileKey, Stanza};

use crate::{Error, Result};

/// The vault key: 1–2 `age` X25519 identities (two during rekey), current first.
pub struct LocalKey {
    keys: Vec<age::x25519::Identity>,
    label: String,
}

impl LocalKey {
    /// Generates a fresh single-key vault key.
    #[must_use]
    pub fn generate() -> Self {
        Self {
            keys: vec![age::x25519::Identity::generate()],
            label: new_label(),
        }
    }

    /// Parses an `age` identity file: `#` comments and blank lines are ignored,
    /// every other line must be an X25519 identity; 1–2 keys.
    ///
    /// # Errors
    ///
    /// [`Error::Corrupt`] on bad lines or the wrong key count.
    pub(crate) fn from_identity_text(text: &SecretString) -> Result<Self> {
        if text.expose_secret().contains('\r') {
            return Err(Error::Corrupt {
                path: "identity file".into(),
                reason: "contains CR",
            });
        }
        let mut keys = Vec::new();
        for line in text.expose_secret().split('\n') {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let identity = age::x25519::Identity::from_str(line).map_err(|_| Error::Corrupt {
                path: "identity file".into(),
                reason: "line is not an X25519 identity",
            })?;
            keys.push(identity);
        }
        if keys.is_empty() || keys.len() > 2 {
            return Err(Error::Corrupt {
                path: "identity file".into(),
                reason: "must contain one or two identities",
            });
        }
        Ok(Self {
            keys,
            label: new_label(),
        })
    }

    /// Number of identities (`1`, or `2` mid-rotation).
    #[must_use]
    pub(crate) fn key_count(&self) -> usize {
        self.keys.len()
    }

    /// A two-key key `[K', self's K]` with a fresh current identity — the
    /// identity file's mid-rotation form. Consumes `self` (which must hold
    /// a single key).
    pub(crate) fn with_new_current(self) -> Self {
        let mut keys = Vec::with_capacity(self.keys.len() + 1);
        keys.push(age::x25519::Identity::generate());
        keys.extend(self.keys);
        Self {
            keys,
            label: new_label(),
        }
    }

    /// The current identity alone — the identity file's post-rotation form.
    /// Consumes `self`.
    pub(crate) fn current_only(self) -> Self {
        let mut keys = self.keys;
        keys.truncate(1);
        Self {
            keys,
            label: new_label(),
        }
    }

    /// Serializes as an `age` identity file (`# ks vault key` header).
    #[must_use]
    pub(crate) fn to_identity_text(&self) -> SecretString {
        let mut out = String::with_capacity(16 + 65 * self.keys.len());
        out.push_str("# ks vault key\n");
        for key in &self.keys {
            out.push_str(key.to_string().expose_secret());
            out.push('\n');
        }
        SecretString::from(out)
    }
}

impl fmt::Debug for LocalKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LocalKey")
            .field("keys", &self.keys.len())
            .finish_non_exhaustive()
    }
}

impl age::Identity for LocalKey {
    fn unwrap_stanza(
        &self,
        stanza: &Stanza,
    ) -> Option<std::result::Result<FileKey, age::DecryptError>> {
        self.keys.iter().find_map(|key| key.unwrap_stanza(stanza))
    }
}

impl age::Recipient for LocalKey {
    fn wrap_file_key(
        &self,
        file_key: &FileKey,
    ) -> std::result::Result<(Vec<Stanza>, HashSet<String>), age::EncryptError> {
        let current = self.keys.first().ok_or_else(|| {
            age::EncryptError::Io(std::io::Error::other("LocalKey without identities"))
        })?;
        let (stanzas, _) = current.to_public().wrap_file_key(file_key)?;
        Ok((stanzas, HashSet::from([self.label.clone()])))
    }
}

fn new_label() -> String {
    let bytes: [u8; 16] = rand::random();
    crate::hex(bytes)
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

    #[test]
    fn identity_text_round_trip() {
        let key = LocalKey::generate();
        let text = key.to_identity_text();
        assert!(
            text.expose_secret()
                .starts_with("# ks vault key\nAGE-SECRET-KEY-1")
        );
        let parsed = LocalKey::from_identity_text(&text).unwrap();
        assert_eq!(parsed.keys.len(), 1);
    }

    #[test]
    fn identity_text_parse() {
        let key = LocalKey::generate().to_identity_text();
        let line = key
            .expose_secret()
            .lines()
            .last()
            .unwrap_or_default()
            .to_owned();
        for (text, ok) in [
            (format!("# c\n\n{line}\n"), true),
            (format!("{line}\n{line}\n"), true),
            (format!("{line}\n{line}\n{line}\n"), false),
            (String::from("# only comments\n\n"), false),
            (String::from("not-a-key\n"), false),
        ] {
            let is_ok = LocalKey::from_identity_text(&SecretString::from(text.clone())).is_ok();
            assert_eq!(is_ok, ok, "{text:?}");
        }
    }

    #[test]
    fn encryptor_rejects_second_recipient() {
        let key = LocalKey::generate();
        let other = age::x25519::Identity::generate().to_public();
        let key_ref: &dyn age::Recipient = &key;
        let other_ref: &dyn age::Recipient = &other;
        assert!(age::Encryptor::with_recipients([key_ref, other_ref].into_iter()).is_err());
    }
}

/// The session key: the vault key either lives in this process (`Local`) or
/// in the session agent (`Agent`, feature `agent`). Both implement
/// `age::Identity` and `age::Recipient` by delegation.
#[derive(Debug)]
pub enum Key {
    /// This process holds the vault key.
    Local(LocalKey),
    /// The vault key is remote, via the session agent.
    #[cfg(feature = "agent")]
    Agent(crate::agent::AgentKey),
}

impl age::Identity for Key {
    fn unwrap_stanza(
        &self,
        stanza: &Stanza,
    ) -> Option<std::result::Result<FileKey, age::DecryptError>> {
        match self {
            Self::Local(k) => k.unwrap_stanza(stanza),
            #[cfg(feature = "agent")]
            Self::Agent(k) => k.unwrap_stanza(stanza),
        }
    }

    fn unwrap_stanzas(
        &self,
        stanzas: &[Stanza],
    ) -> Option<std::result::Result<FileKey, age::DecryptError>> {
        match self {
            Self::Local(k) => k.unwrap_stanzas(stanzas),
            #[cfg(feature = "agent")]
            Self::Agent(k) => k.unwrap_stanzas(stanzas),
        }
    }
}

impl age::Recipient for Key {
    fn wrap_file_key(
        &self,
        file_key: &FileKey,
    ) -> std::result::Result<(Vec<Stanza>, HashSet<String>), age::EncryptError> {
        match self {
            Self::Local(k) => k.wrap_file_key(file_key),
            #[cfg(feature = "agent")]
            Self::Agent(k) => k.wrap_file_key(file_key),
        }
    }
}
