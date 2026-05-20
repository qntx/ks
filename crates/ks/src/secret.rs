//! Secret value data model.
//!
//! A [`Secret`] is the deserialised payload of an encrypted `.age` file. It
//! carries the primary value, optional named fields, free-form notes,
//! timestamps and a discriminator (`kind`) for special types such as TOTP.

use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

/// Current on-disk secret format version.
pub const SECRET_FORMAT_VERSION: u32 = 1;

/// Discriminator for the kind of secret stored in [`Secret::value`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// A plain secret value (password, API token, …).
    #[default]
    Secret,
    /// A TOTP `otpauth://` URL or raw base32 secret; used by `ks otp`.
    Totp,
}

/// In-memory representation of a secret.
///
/// `value` and entries of `fields` are wrapped in [`Zeroizing`] so they are
/// scrubbed from memory on drop.
#[derive(Debug, Clone)]
pub struct Secret {
    /// The primary secret value.
    pub value: Zeroizing<String>,
    /// Additional named fields (e.g. `region`, `username`).
    pub fields: BTreeMap<String, Zeroizing<String>>,
    /// Human-readable note. Not treated as secret.
    pub note: String,
    /// What kind of value this is. `Totp` triggers TOTP generation in `ks otp`.
    pub kind: Kind,
    /// Unix timestamp (seconds) when this secret was created.
    pub created_at: u64,
    /// Unix timestamp (seconds) when this secret was last updated.
    pub updated_at: u64,
}

impl Secret {
    /// Creates a new plain secret with the given primary value.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        let now = unix_now();
        Self {
            value: Zeroizing::new(value.into()),
            fields: BTreeMap::new(),
            note: String::new(),
            kind: Kind::Secret,
            created_at: now,
            updated_at: now,
        }
    }

    /// Marks this secret as a TOTP source (otpauth URL or raw base32).
    #[must_use]
    pub const fn into_totp(mut self) -> Self {
        self.kind = Kind::Totp;
        self
    }

    /// Attaches a note and returns `self` (builder pattern).
    #[must_use]
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = note.into();
        self
    }

    /// Inserts or updates an additional field and bumps `updated_at`.
    pub fn set_field(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.fields.insert(key.into(), Zeroizing::new(value.into()));
        self.updated_at = unix_now();
    }

    /// Returns the value of an additional field, or `None` if absent.
    #[must_use]
    pub fn field(&self, key: &str) -> Option<&str> {
        self.fields.get(key).map(|v| v.as_str())
    }
}

/// Wire format actually serialised to disk inside the encrypted `.age` file.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Wire {
    pub(crate) v: u32,
    pub(crate) value: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) fields: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(crate) note: String,
    #[serde(default)]
    pub(crate) kind: Kind,
    pub(crate) created_at: u64,
    pub(crate) updated_at: u64,
}

impl From<&Secret> for Wire {
    fn from(s: &Secret) -> Self {
        Self {
            v: SECRET_FORMAT_VERSION,
            value: (*s.value).clone(),
            fields: s
                .fields
                .iter()
                .map(|(k, v)| (k.clone(), (**v).clone()))
                .collect(),
            note: s.note.clone(),
            kind: s.kind,
            created_at: s.created_at,
            updated_at: s.updated_at,
        }
    }
}

impl From<Wire> for Secret {
    fn from(w: Wire) -> Self {
        Self {
            value: Zeroizing::new(w.value),
            fields: w
                .fields
                .into_iter()
                .map(|(k, v)| (k, Zeroizing::new(v)))
                .collect(),
            note: w.note,
            kind: w.kind,
            created_at: w.created_at,
            updated_at: w.updated_at,
        }
    }
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_preserves_fields() {
        let mut s = Secret::new("ghp_token").with_note("PAT");
        s.set_field("scope", "repo,workflow");
        let wire = Wire::from(&s);
        let json = serde_json::to_vec(&wire).expect("serialise");
        let parsed: Wire = serde_json::from_slice(&json).expect("parse");
        let restored: Secret = parsed.into();
        assert_eq!(&*restored.value, "ghp_token");
        assert_eq!(restored.note, "PAT");
        assert_eq!(restored.field("scope"), Some("repo,workflow"));
        assert_eq!(restored.kind, Kind::Secret);
    }

    #[test]
    fn totp_kind_roundtrips() {
        let s = Secret::new("otpauth://totp/...").into_totp();
        let json = serde_json::to_vec(&Wire::from(&s)).expect("serialise");
        let parsed: Wire = serde_json::from_slice(&json).expect("parse");
        assert_eq!(parsed.kind, Kind::Totp);
    }
}
