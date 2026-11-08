//! `ks` — a personal local secret vault with an age-compatible on-disk format.
//!
//! Every entry is a standard `age` file with exactly one X25519 recipient
//! stanza, encrypted to the recipient `R` of the vault key `K`. `R` exists
//! only in memory: files are unforgeable without it (single-recipient
//! authentication), and it is never written to disk, printed, or transmitted.
#![forbid(unsafe_code)]

mod entry;
mod error;
mod header;
mod key;
mod name;
mod passphrase;

#[cfg(test)]
use proptest as _;
#[cfg(test)]
use serde_json as _;

fn hex<const N: usize>(bytes: [u8; N]) -> String {
    let mut out = String::with_capacity(2 * N);
    for b in bytes {
        out.push(char::from_digit(u32::from(b >> 4), 16).unwrap_or('0'));
        out.push(char::from_digit(u32::from(b & 0xf), 16).unwrap_or('0'));
    }
    out
}

pub use entry::{Entry, Kind, Text};
pub use error::{Error, ErrorCode, Result};
pub use key::LocalKey;
pub use name::Name;
pub use passphrase::check_new_passphrase;
