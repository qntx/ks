//! OS keychain device unlock (feature `keychain`).
//!
//! The vault identity text lives in the platform credential store under
//! service `ks`, account `<vault id>`. macOS uses the legacy keychain store
//! (works for unsigned CLI binaries), Windows the native credential store,
//! Linux Secret Service via the pure-Rust zbus store.
//!
//! The platform default store is installed once in this library; tests
//! install [`keyring_core::mock::MockCredentialStore`] instead.

use std::sync::Arc;

use age::secrecy::{ExposeSecret as _, SecretString};
use zeroize::Zeroizing;

use crate::unlock::Probe;
use crate::{Error, LocalKey, Result, Session, Vault};

/// Keychain service name shared by all vaults; the account is the vault id.
const SERVICE: &str = "ks";

fn entry(vault: &Vault) -> Result<keyring_core::Entry> {
    ensure_store()?;
    keyring_core::Entry::new(SERVICE, &crate::local::id(vault)).map_err(|e| {
        Error::Unavailable(format!("cannot open the OS keychain: {e}"))
    })
}

/// Installs the platform default store once (first use wins; a test may
/// install the mock store beforehand).
fn ensure_store() -> Result<()> {
    if keyring_core::get_default_store().is_some() {
        return Ok(());
    }
    keyring_core::set_default_store(platform_store()?);
    Ok(())
}

#[cfg(target_os = "macos")]
fn platform_store() -> Result<Arc<keyring_core::CredentialStore>> {
    let store: Arc<keyring_core::CredentialStore> =
        apple_native_keyring_store::keychain::Store::new()
            .map_err(|e| Error::Unavailable(format!("macOS keychain store: {e}")))?;
    Ok(store)
}

#[cfg(windows)]
fn platform_store() -> Result<Arc<keyring_core::CredentialStore>> {
    let store: Arc<keyring_core::CredentialStore> = windows_native_keyring_store::Store::new()
        .map_err(|e| Error::Unavailable(format!("Windows credential store: {e}")))?;
    Ok(store)
}

#[cfg(target_os = "linux")]
fn platform_store() -> Result<Arc<keyring_core::CredentialStore>> {
    let store: Arc<keyring_core::CredentialStore> =
        zbus_secret_service_keyring_store::Store::new()
            .map_err(|e| Error::Unavailable(format!("Secret Service store: {e}")))?;
    Ok(store)
}

#[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
fn platform_store() -> Result<Arc<keyring_core::CredentialStore>> {
    Err(Error::Unavailable(
        "no OS keychain store on this platform".into(),
    ))
}

/// Whether a keychain entry exists for this vault.
///
/// # Errors
///
/// [`Error::Unavailable`] when the store itself cannot be reached.
pub fn is_enabled(vault: &Vault) -> Result<bool> {
    match entry(vault)?.get_secret() {
        Ok(_) => Ok(true),
        Err(keyring_core::Error::NoEntry) => Ok(false),
        Err(e) => Err(Error::Unavailable(format!("keychain read failed: {e}"))),
    }
}

/// Stores the vault key in the OS keychain. `session` must hold the local
/// key (a passphrase/Touch-ID unlock, never the agent).
///
/// # Errors
///
/// [`Error::InvalidInput`] without a local key; [`Error::Unavailable`] when
/// the store cannot be reached or the read-back check fails.
#[cfg_attr(
    not(target_os = "linux"),
    allow(
        clippy::unnecessary_wraps,
        reason = "the warning list is populated on linux only; Vec is the contract"
    )
)]
pub fn enable(session: &Session) -> Result<Vec<String>> {
    let Some(text) = session.agent_identity() else {
        return Err(Error::InvalidInput(
            "enabling the keychain needs the master passphrase, Touch ID or an existing keychain"
                .into(),
        ));
    };
    let entry = entry(session.vault())?;
    entry
        .set_secret(text.expose_secret().as_bytes())
        .map_err(|e| Error::Unavailable(format!("keychain write failed: {e}")))?;
    let read_back = entry
        .get_secret()
        .map_err(|e| Error::Unavailable(format!("keychain read-back failed: {e}")))?;
    if read_back != text.expose_secret().as_bytes() {
        return Err(Error::Unavailable(
            "keychain read-back does not match the stored key".into(),
        ));
    }
    let warnings = Vec::new();
    #[cfg(target_os = "linux")]
    let warnings = vec![
        "Secret Service is readable by any process of this user while the desktop session is unlocked"
            .to_owned(),
    ];
    Ok(warnings)
}

/// Deletes the keychain entry; `true` if one existed.
///
/// # Errors
///
/// [`Error::Unavailable`] when the store cannot be reached.
pub fn disable(vault: &Vault) -> Result<bool> {
    match entry(vault)?.delete_credential() {
        Ok(()) => Ok(true),
        Err(keyring_core::Error::NoEntry) => Ok(false),
        Err(e) => Err(Error::Unavailable(format!("keychain delete failed: {e}"))),
    }
}

/// Unlock-chain probe: reads the keychain entry and yields the key for the
/// check-file verification. Every failure is a soft skip — access denied,
/// read errors and an unavailable store are never treated as stale.
pub(crate) fn probe(vault: &Vault) -> Probe {
    let entry = match entry(vault) {
        Ok(e) => e,
        Err(e) => return Probe::Skip(format!("OS keychain could not be read: {e}")),
    };
    let secret = match entry.get_secret() {
        Ok(s) => Zeroizing::new(s),
        Err(keyring_core::Error::NoEntry) => return Probe::Absent,
        Err(e) => return Probe::Skip(format!("OS keychain could not be read: {e}")),
    };
    let Ok(text) = String::from_utf8(secret.to_vec()) else {
        return Probe::Skip("keychain entry is not a vault key".to_owned());
    };
    LocalKey::from_identity_text(&SecretString::from(text)).map_or_else(
        |_| Probe::Skip("keychain entry is not a vault key".to_owned()),
        Probe::Key,
    )
}
