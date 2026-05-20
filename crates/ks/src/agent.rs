//! OS-keyring backed session cache for unlocked identities.
//!
//! Caches the **bech32 secret key** (not the user's passphrase) keyed by store
//! identifier. Each entry carries an expiry timestamp; entries past their TTL
//! are treated as absent and deleted on first read.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use age::x25519;
use keyring::Entry;
use zeroize::Zeroizing;

use crate::config::store_id;
use crate::error::{Error, Result};
use crate::identity;

const SERVICE: &str = "ks";

/// Tries to fetch a valid (non-expired) cached identity for `store_dir`.
///
/// On hit and TTL OK: returns `Some(identity)`.
/// On miss, expired, parse error or any keyring failure: returns `None`
/// (silently — the keyring may simply be unavailable).
#[must_use]
pub fn get(store_dir: &Path) -> Option<x25519::Identity> {
    let entry = entry_for(store_dir)?;
    let raw = entry.get_password().ok()?;
    let raw = Zeroizing::new(raw);
    let (expiry, key) = parse_payload(&raw)?;
    if expiry <= now_secs() {
        let _ = entry.delete_credential();
        return None;
    }
    identity::parse(&key).ok()
}

/// Caches `identity` for `store_dir` with `ttl_secs` lifetime.
///
/// Silently no-ops if the OS keyring is unavailable.
///
/// # Errors
/// Returns [`Error::Keyring`] only for unexpected keyring backend failures.
pub fn put(store_dir: &Path, identity: &x25519::Identity, ttl_secs: u64) -> Result<()> {
    let Some(entry) = entry_for(store_dir) else {
        return Ok(());
    };
    let key = identity::to_secret_string(identity);
    let payload = format!("{}|{}", now_secs().saturating_add(ttl_secs), key.as_str());
    let payload = Zeroizing::new(payload);
    entry.set_password(payload.as_str())?;
    Ok(())
}

/// Removes any cached identity for `store_dir`.
///
/// # Errors
/// Returns [`Error::Keyring`] for unexpected backend failures. A missing entry
/// is treated as success.
pub fn clear(store_dir: &Path) -> Result<()> {
    let Some(entry) = entry_for(store_dir) else {
        return Ok(());
    };
    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(Error::Keyring(e.to_string())),
    }
}

fn entry_for(store_dir: &Path) -> Option<Entry> {
    let account = format!("identity:{}", store_id(store_dir));
    Entry::new(SERVICE, &account).ok()
}

fn parse_payload(payload: &str) -> Option<(u64, String)> {
    let (head, tail) = payload.split_once('|')?;
    let expiry: u64 = head.parse().ok()?;
    Some((expiry, tail.to_owned()))
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_payload_works() {
        let p = "1700000000|AGE-SECRET-KEY-1XXX";
        let (e, k) = parse_payload(p).expect("parse");
        assert_eq!(e, 1700000000);
        assert_eq!(k, "AGE-SECRET-KEY-1XXX");
    }

    #[test]
    fn parse_payload_rejects_malformed() {
        assert!(parse_payload("no-delim").is_none());
        assert!(parse_payload("abc|key").is_none());
    }
}
