use thiserror::Error;

/// All errors that can occur in the `ks` library.
#[derive(Debug, Error)]
pub enum Error {
    /// Vault file does not exist; run `ks init` to create one.
    #[error("vault not found; run `ks init` to create one")]
    VaultNotFound,

    /// Vault file already exists at the given path.
    #[error("vault already exists at {0}")]
    VaultExists(std::path::PathBuf),

    /// Passphrase was incorrect.
    #[error("wrong passphrase")]
    WrongPassphrase,

    /// No secret was found at the given path.
    #[error("secret not found: {0}")]
    SecretNotFound(String),

    /// A secret already exists at the given path.
    #[error("secret already exists: {0}")]
    SecretExists(String),

    /// The secret path is invalid (empty, bad characters, etc.).
    #[error("invalid path: {0}")]
    InvalidPath(String),

    /// An I/O error occurred.
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),

    /// An encryption error occurred.
    #[error("encryption failed: {0}")]
    Encrypt(String),

    /// A decryption error occurred.
    #[error("decryption failed: {0}")]
    Decrypt(String),

    /// A JSON serialization/deserialization error occurred.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    /// An OS keyring error occurred.
    #[error("keyring error: {0}")]
    Keyring(String),

    /// Could not determine the home/data directory for the current user.
    #[error("could not determine home directory")]
    NoHomeDir,
}

/// Convenience `Result` type for this library.
pub type Result<T> = std::result::Result<T, Error>;
