use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

/// A secret entry containing a primary value and optional metadata.
#[derive(Debug, Clone)]
pub struct Secret {
    /// The primary secret value (e.g. a token, password, or key).
    pub value: Zeroizing<String>,
    /// Additional named fields (e.g. AWS `secret`, `region`).
    pub fields: HashMap<String, Zeroizing<String>>,
    /// Human-readable description.
    pub note: String,
    /// Unix timestamp (seconds) when this secret was created.
    pub created_at: u64,
    /// Unix timestamp (seconds) when this secret was last updated.
    pub updated_at: u64,
}

impl Secret {
    /// Creates a new secret with the given primary value.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        let now = unix_now();
        Self {
            value: Zeroizing::new(value.into()),
            fields: HashMap::new(),
            note: String::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Attaches a note and returns `self` (builder pattern).
    #[must_use]
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = note.into();
        self
    }

    /// Inserts or updates an additional field, updating `updated_at`.
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

#[derive(Serialize, Deserialize)]
pub(crate) struct SecretData {
    pub(crate) value: String,
    #[serde(default)]
    pub(crate) fields: HashMap<String, String>,
    #[serde(default)]
    pub(crate) note: String,
    pub(crate) created_at: u64,
    pub(crate) updated_at: u64,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct VaultData {
    pub(crate) version: u32,
    pub(crate) secrets: HashMap<String, SecretData>,
}

impl From<SecretData> for Secret {
    fn from(d: SecretData) -> Self {
        Self {
            value: Zeroizing::new(d.value),
            fields: d
                .fields
                .into_iter()
                .map(|(k, v)| (k, Zeroizing::new(v)))
                .collect(),
            note: d.note,
            created_at: d.created_at,
            updated_at: d.updated_at,
        }
    }
}

impl From<&Secret> for SecretData {
    fn from(s: &Secret) -> Self {
        Self {
            value: (*s.value).clone(),
            fields: s
                .fields
                .iter()
                .map(|(k, v)| (k.clone(), (**v).clone()))
                .collect(),
            note: s.note.clone(),
            created_at: s.created_at,
            updated_at: s.updated_at,
        }
    }
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
