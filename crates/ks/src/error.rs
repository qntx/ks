//! Library-wide error and result types.

use std::path::PathBuf;

use thiserror::Error;

/// Convenience `Result` alias used throughout the `ks` library.
pub type Result<T> = std::result::Result<T, Error>;

/// All errors that can occur in the `ks` library.
#[expect(
    clippy::error_impl_error,
    reason = "naming a crate's primary error `Error` is idiomatic in Rust libraries"
)]
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    /// The store has not been initialised; run `ks init`.
    #[error("store not initialised at {0}; run `ks init`")]
    StoreNotFound(PathBuf),

    /// A store already exists at the given path.
    #[error("store already exists at {0}")]
    StoreExists(PathBuf),

    /// The identity file does not exist.
    #[error("identity file not found at {0}")]
    IdentityNotFound(PathBuf),

    /// The identity file already exists.
    #[error("identity file already exists at {0}")]
    IdentityExists(PathBuf),

    /// The recipients file does not exist or is empty.
    #[error("no recipients configured at {0}")]
    NoRecipients(PathBuf),

    /// A passphrase was rejected by the age identity file.
    #[error("incorrect passphrase")]
    WrongPassphrase,

    /// No secret exists at the given logical path.
    #[error("secret not found: {0}")]
    SecretNotFound(String),

    /// A secret already exists at the given logical path.
    #[error("secret already exists: {0}")]
    SecretExists(String),

    /// The supplied logical path is invalid (empty, bad characters, ...).
    #[error("invalid secret path: {0}")]
    InvalidPath(String),

    /// The supplied age recipient could not be parsed.
    #[error("invalid age recipient: {0}")]
    InvalidRecipient(String),

    /// A TOTP secret/URL was malformed.
    #[error("invalid TOTP secret: {0}")]
    InvalidTotp(String),

    /// An I/O error occurred.
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),

    /// An age encryption error.
    #[error("encryption failed: {0}")]
    Encrypt(String),

    /// An age decryption error not caused by a wrong passphrase.
    #[error("decryption failed: {0}")]
    Decrypt(String),

    /// A JSON (de)serialisation error.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    /// A TOML (de)serialisation error.
    #[error("config parse error: {0}")]
    Toml(String),

    /// OS keyring failure.
    #[error("keyring error: {0}")]
    Keyring(String),

    /// Could not determine a default user directory (home/data/config).
    #[error("could not determine user directory")]
    NoUserDir,

    /// An external command failed.
    #[error("command `{cmd}` failed with status {status}: {stderr}")]
    Command {
        /// The command that was attempted.
        cmd: String,
        /// The numeric exit status.
        status: i32,
        /// Captured stderr output.
        stderr: String,
    },
}

impl From<toml::de::Error> for Error {
    fn from(value: toml::de::Error) -> Self {
        Self::Toml(value.to_string())
    }
}

impl From<toml::ser::Error> for Error {
    fn from(value: toml::ser::Error) -> Self {
        Self::Toml(value.to_string())
    }
}

impl From<keyring::Error> for Error {
    fn from(value: keyring::Error) -> Self {
        Self::Keyring(value.to_string())
    }
}
