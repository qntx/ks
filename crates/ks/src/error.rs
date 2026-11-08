//! Error types shared by the whole library.

use std::path::PathBuf;

/// Result alias for the `ks` library.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors returned by the `ks` library.
///
/// Messages never contain secret material; they may contain paths and names.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
#[allow(
    clippy::error_impl_error,
    reason = "the crate exports a single error type named Error"
)]
pub enum Error {
    /// A name violates the naming rules.
    #[error("invalid name {name:?}: {reason}")]
    InvalidName {
        /// The rejected name.
        name: String,
        /// The rule that was violated.
        reason: &'static str,
    },
    /// Caller supplied invalid input (not a vault name).
    #[error("invalid input: {0}")]
    InvalidInput(String),
    /// A file under the vault is corrupt or hostile.
    #[error("corrupt file {path:?}: {reason}")]
    Corrupt {
        /// Path of the offending file.
        path: PathBuf,
        /// What is wrong with it.
        reason: &'static str,
    },
    /// A new passphrase failed the policy checks.
    #[error("weak passphrase: {0}")]
    WeakPassphrase(String),
    /// The named entry does not exist.
    #[error("entry not found: {0}")]
    NotFound(String),
    /// The target already exists.
    #[error("already exists: {0}")]
    Exists(String),
    /// No usable key source was available.
    #[error("vault is locked")]
    Locked,
    /// The passphrase did not decrypt the identity file.
    #[error("wrong passphrase")]
    WrongPassphrase,
    /// An internal invariant failed (a program defect, not user input).
    #[error("internal error: {0}")]
    Internal(&'static str),
    /// A filesystem operation failed.
    #[error("I/O error on {path:?}: {source}")]
    Io {
        /// Path involved in the operation.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
}

impl Error {
    /// Machine-readable code for this error.
    #[must_use]
    pub const fn code(&self) -> ErrorCode {
        match self {
            Self::InvalidName { .. } | Self::InvalidInput(_) => ErrorCode::Usage,
            Self::Corrupt { .. } => ErrorCode::Corrupt,
            Self::WeakPassphrase(_) => ErrorCode::WeakPassphrase,
            Self::NotFound(_) => ErrorCode::NotFound,
            Self::Exists(_) => ErrorCode::Exists,
            Self::Locked => ErrorCode::Locked,
            Self::WrongPassphrase => ErrorCode::WrongPassphrase,
            Self::Io { .. } => ErrorCode::Io,
            Self::Internal(_) => ErrorCode::Internal,
        }
    }
}

/// Stable machine-readable error codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErrorCode {
    /// Bad usage: invalid names or arguments.
    Usage,
    /// A vault file is corrupt or hostile.
    Corrupt,
    /// New passphrase failed policy.
    WeakPassphrase,
    /// Entry not found.
    NotFound,
    /// Target already exists.
    Exists,
    /// No key source available.
    Locked,
    /// Wrong passphrase.
    WrongPassphrase,
    /// An external capability is unavailable.
    Unavailable,
    /// I/O failure.
    Io,
    /// The user cancelled an operation.
    Cancelled,
    /// Internal error.
    Internal,
}

impl ErrorCode {
    /// The `snake_case` code string.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Usage => "usage",
            Self::Corrupt => "corrupt",
            Self::WeakPassphrase => "weak_passphrase",
            Self::NotFound => "not_found",
            Self::Exists => "exists",
            Self::Locked => "locked",
            Self::WrongPassphrase => "wrong_passphrase",
            Self::Unavailable => "unavailable",
            Self::Io => "io",
            Self::Cancelled => "cancelled",
            Self::Internal => "internal",
        }
    }
}
