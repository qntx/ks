use keyring::Entry;
use zeroize::Zeroizing;

use crate::error::{Error, Result};

const SERVICE: &str = "ks";
const ACCOUNT: &str = "passphrase";

/// Attempts to read the cached passphrase from the OS keyring.
///
/// Returns `None` if no session is active or the keyring is unavailable.
#[must_use]
pub fn get_passphrase() -> Option<Zeroizing<String>> {
    let entry = Entry::new(SERVICE, ACCOUNT).ok()?;
    entry.get_password().ok().map(Zeroizing::new)
}

/// Stores the passphrase in the OS keyring for the current session.
///
/// # Errors
/// Returns [`Error::Keyring`] if the OS keyring is unavailable or access is denied.
pub fn set_passphrase(passphrase: &str) -> Result<()> {
    let entry =
        Entry::new(SERVICE, ACCOUNT).map_err(|e| Error::Keyring(e.to_string()))?;
    entry
        .set_password(passphrase)
        .map_err(|e| Error::Keyring(e.to_string()))
}

/// Removes the cached passphrase from the OS keyring (locks the vault session).
///
/// # Errors
/// Returns [`Error::Keyring`] on an unexpected keyring error.
/// A missing entry is silently ignored.
pub fn clear() -> Result<()> {
    let entry =
        Entry::new(SERVICE, ACCOUNT).map_err(|e| Error::Keyring(e.to_string()))?;
    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(Error::Keyring(e.to_string())),
    }
}
