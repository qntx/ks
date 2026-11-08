//! `ks` — a personal local secret vault with an age-compatible on-disk format.
//!
//! Every entry is a standard `age` file with exactly one X25519 recipient
//! stanza, encrypted to the recipient `R` of the vault key `K`. `R` exists
//! only in memory: files are unforgeable without it (single-recipient
//! authentication), and it is never written to disk, printed, or transmitted.
#![forbid(unsafe_code)]

mod entry;
mod error;
mod name;

#[cfg(test)]
use proptest as _;
#[cfg(test)]
use serde_json as _;

pub use entry::{Entry, Kind, Text};
pub use error::{Error, ErrorCode, Result};
pub use name::Name;
