//! # ks — Key Store
//!
//! A secure local secret management library.
//!
//! Secrets are stored in a single age-encrypted vault file and accessed
//! via a simple path-namespaced API.  An optional OS-keyring session cache
//! means users only need to type their passphrase once per login session.
//!
//! ## Quick start
//!
//! ```no_run
//! use ks::{Config, Vault};
//! use zeroize::Zeroizing;
//!
//! let config = Config::load().unwrap();
//! let pass = Zeroizing::new("my-passphrase".to_owned());
//!
//! // First time: create vault
//! let mut vault = Vault::create(&config, pass).unwrap();
//!
//! // Store a secret
//! vault.set("github/token", ks::Secret::new("ghp_abc123")).unwrap();
//! vault.save().unwrap();
//! ```

/// Runtime configuration and vault path resolution.
pub mod config;
/// Library-wide error and result types.
pub mod error;
/// Cryptographically random secret generation.
pub mod generate;
/// OS-keyring-backed passphrase session cache.
pub mod session;
/// Secret data types used throughout the library.
pub mod store;
/// Encrypted vault: open, save, and CRUD operations.
pub mod vault;

pub use config::Config;
pub use error::{Error, Result};
pub use store::Secret;
pub use vault::Vault;
