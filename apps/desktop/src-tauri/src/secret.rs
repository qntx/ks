//! The only type allowed to carry secrets across the IPC boundary.

use serde::{Deserialize, Serialize};
use specta::Type;
use zeroize::Zeroize as _;

/// A secret string. Serialized as a plain string for IPC (the `WebView`
/// only sees it when a whitelisted command returns or accepts one);
/// zeroized on drop.
#[derive(Debug, Clone, Type)]
#[specta(transparent)]
pub(crate) struct SecretText(pub String);

impl Drop for SecretText {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl Serialize for SecretText {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for SecretText {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(Self(String::deserialize(d)?))
    }
}

impl SecretText {
    /// Wraps a plain string as secret.
    pub(crate) fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }
    /// Borrows the secret content.
    pub(crate) fn expose(&self) -> &str {
        &self.0
    }
}
