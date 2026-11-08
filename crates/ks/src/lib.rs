//! `ks` — a personal local secret vault with an age-compatible on-disk format.
//!
//! Every entry is a standard `age` file with exactly one X25519 recipient
//! stanza, encrypted to the recipient `R` of the vault key `K`. `R` exists
//! only in memory: files are unforgeable without it (single-recipient
//! authentication), and it is never written to disk, printed, or transmitted.
//!
//! Vault layout (see `design/format.mdx`):
//! - `.identity.age` — `K` as an `age` identity file, `age` scrypt-encrypted
//!   under the master passphrase (work factor 2^18; decryption cap 2^20).
//! - `.check.age` — `ks1 check <sha256 of .identity.age>` encrypted to `R`;
//!   device unlock sources must verify it.
//! - `<name>.age` — entries whose plaintext starts `ks1 <kind> <name>`,
//!   binding the file to its path.
//! - `.lock` — the write lock for all mutating operations.
#![forbid(unsafe_code)]

mod entry;
mod error;
mod header;
mod key;
mod name;
mod passphrase;
mod unlock;
mod vault;

pub mod pwgen;
pub mod totp;

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
pub use vault::{Session, Vault};
