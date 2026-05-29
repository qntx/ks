//! # ks — Key Store
//!
//! A modern, local-first, git-friendly secret manager built on the
//! [`age`](https://age-encryption.org/) encryption format.
//!
//! ## Architecture
//!
//! - **Identity** (`identity.age`): a single X25519 secret key, encrypted to
//!   the user's passphrase with age scrypt mode. Stays local.
//! - **Recipients** (`store/.age-recipients`): a plaintext list of `age1…`
//!   public keys allowed to decrypt this store. Git-synced with the secrets.
//! - **Secrets** (`store/<path>.age`): each secret is its own
//!   recipient-encrypted age file whose plaintext is just text — the first line
//!   is the value, `key: value` lines are fields. `age -d secret.age` is
//!   human-readable and interoperable with the `age` / `rage` CLIs.
//!
//! ## Asymmetry
//!
//! Encryption needs only the public recipients, so writing secrets never
//! prompts for a passphrase. Only reading (and rotating recipients) requires
//! the unlocked [`x25519::Identity`].
//!
//! ```no_run
//! use age::secrecy::SecretString;
//! use ks::{Config, Secret, Store, crypto};
//!
//! fn main() -> ks::Result<()> {
//!     let config = Config::load()?;
//!     let pp = SecretString::from("hunter2".to_owned());
//!     let id = crypto::create_identity(&config.identity_path, pp)?;
//!     let store = Store::create(config, &id, &[])?;
//!
//!     store.set("github/token", &Secret::new("ghp_xxx\nuser: alice"))?; // no unlock
//!     let token = store.get("github/token", &id)?;
//!     assert_eq!(token.password(), "ghp_xxx");
//!     Ok(())
//! }
//! ```

/// Runtime configuration (filesystem paths).
pub mod config;
/// age encryption primitives, identity file, and recipient list.
pub mod crypto;
/// Library-wide error and result types.
pub mod error;
/// Thin wrapper over the system `git` binary.
pub mod git;
/// Logical secret path validation.
pub mod path;
/// Cryptographically-random secret generation.
pub mod pwgen;
/// Plaintext secret model.
pub mod secret;
/// The encrypted secret store.
pub mod store;
/// RFC 6238 TOTP generation.
pub mod totp;

pub use age::x25519;
pub use config::Config;
pub use error::{Error, Result};
pub use secret::Secret;
pub use store::Store;
