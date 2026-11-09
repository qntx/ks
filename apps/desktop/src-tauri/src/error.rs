//! Command error type — `code` mirrors `ks::ErrorCode` strings.

use serde::Serialize;
use specta::Type;

/// Error returned by commands; the frontend branches on `code`.
#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppError {
    /// `ks::ErrorCode` string: locked, corrupt, not_found, exists,
    /// usage, conflict, unavailable, io, weak_passphrase,
    /// wrong_passphrase, cancelled, internal.
    pub code: String,
    /// Human-readable message.
    pub message: String,
}

impl AppError {
    ///  — unexpected backend failure (plugin calls, joins).
    pub(crate) fn internal(message: impl Into<String>) -> Self {
        Self {
            code: "internal".into(),
            message: message.into(),
        }
    }
}

impl From<ks::Error> for AppError {
    fn from(e: ks::Error) -> Self {
        Self {
            code: code_str(e.code()).to_owned(),
            message: e.to_string(),
        }
    }
}

const fn code_str(c: ks::ErrorCode) -> &'static str {
    match c {
        ks::ErrorCode::Usage => "usage",
        ks::ErrorCode::Corrupt => "corrupt",
        ks::ErrorCode::WeakPassphrase => "weak_passphrase",
        ks::ErrorCode::NotFound => "not_found",
        ks::ErrorCode::Exists => "exists",
        ks::ErrorCode::Locked => "locked",
        ks::ErrorCode::WrongPassphrase => "wrong_passphrase",
        ks::ErrorCode::Unavailable => "unavailable",
        ks::ErrorCode::Io => "io",
        ks::ErrorCode::Cancelled => "cancelled",
        ks::ErrorCode::Conflict => "conflict",
        ks::ErrorCode::Internal => "internal",
        _ => "internal",
    }
}

impl From<String> for AppError {
    fn from(message: String) -> Self {
        Self {
            code: "internal".to_owned(),
            message,
        }
    }
}
