//! # ks — Key Store
//!
//! A modern, local-first, git-friendly secret manager built on the
//! [`age`](https://age-encryption.org/) encryption format.
//!
//! ## Architecture
//!
//! - **Identity** (`identity.age`): a single X25519 secret key, encrypted
//!   to the user's passphrase using age scrypt mode. Stays local.
//! - **Recipients** (`store/.recipients`): plaintext list of age public keys
//!   allowed to decrypt this store. Lives inside the store, safe to git-sync.
//! - **Secrets** (`store/<path>.age`): each secret is its own
//!   recipient-encrypted age file containing a small JSON blob.
//!
//! ## Quick start
//!
//! ```no_run
//! use age::secrecy::SecretString;
//! use ks::{Config, Secret, Store, identity};
//!
//! let config = Config::load().expect("load config");
//! let pp = SecretString::from("hunter2".to_owned());
//! let id = identity::create(&config.identity_path, pp).expect("init identity");
//! let store = Store::create(config, id, &[]).expect("init store");
//!
//! store.set("github/token", &Secret::new("ghp_xxx")).expect("set");
//! let token = store.get("github/token").expect("get");
//! assert_eq!(&*token.value, "ghp_xxx");
//! ```

/// OS-keyring backed session cache.
pub mod agent;
/// Runtime configuration (paths, tunables).
pub mod config;
/// Low-level age encryption primitives.
pub mod crypto;
/// Library-wide error and result types.
pub mod error;
/// Thin wrapper over the system `git` binary.
pub mod git;
/// Age identity file management.
pub mod identity;
/// Logical secret path validation.
pub mod path;
/// Cryptographically-random secret generation.
pub mod pwgen;
/// Recipient list management.
pub mod recipient;
/// Secret value data model.
pub mod secret;
/// The encrypted secret store.
pub mod store;
/// RFC 6238 TOTP generation.
pub mod totp;

pub use age::x25519;
pub use config::{Config, Tunables};
pub use error::{Error, Result};
pub use secret::{Kind, Secret};
pub use store::Store;
