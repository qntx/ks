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

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::panic,
        clippy::assert_is_empty,
        reason = "test harness: panicking fixtures are intentional"
    )]

    use age::secrecy::SecretString;

    use super::*;
    use crate::Name;

    const PASS: &str = "correct horse battery staple fixture";

    fn vault() -> (tempfile::TempDir, Session) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("v");
        let session = Vault::create(&root, &SecretString::from(PASS)).unwrap();
        (dir, session)
    }

    fn source() -> SecretString {
        SecretString::from(PASS)
    }

    fn mock() -> std::sync::Arc<keyring_core::mock::Store> {
        let store = keyring_core::mock::Store::new().unwrap();
        keyring_core::set_default_store(store.clone() as std::sync::Arc<keyring_core::CredentialStore>);
        store
    }

    /// All keychain tests share one global store, so they run in one fn.
    #[test]
    fn keychain_roundtrip_stale_and_errors() {
        mock();
        let (_d1, s1) = vault();
        // enable -> is_enabled -> probe -> unlock without passphrase
        drop(enable(&s1).unwrap());
        assert!(is_enabled(s1.vault()).unwrap());
        let s2 = s1.vault().unlock_without_agent(None).unwrap();
        s2.get(&Name::parse("x").unwrap()).unwrap_err();
        // disable
        assert!(disable(s1.vault()).unwrap());
        assert!(!is_enabled(s1.vault()).unwrap());

        // stale: another vault's key text in this vault's keychain entry
        let (_d2, other) = vault();
        let other_text = other.agent_identity().unwrap();
        let e = keyring_core::Entry::new(SERVICE, &crate::local::id(s1.vault())).unwrap();
        e.set_secret(other_text.expose_secret().as_bytes()).unwrap();
        let s3 = s1
            .vault()
            .unlock_without_agent(Some(&|| Ok(source())))
            .unwrap();
        assert!(
            s3.warnings()
                .iter()
                .any(|w| w.contains("keychain")),
            "expected a stale-keychain warning, got {:?}",
            s3.warnings()
        );
        // stale entry was removed after the passphrase unlock
        assert!(!is_enabled(s1.vault()).unwrap());

        // read failure: skipped, never removed
        let (_d3, s4) = vault();
        enable(&s4).unwrap();
        let e4 = keyring_core::Entry::new(SERVICE, &crate::local::id(s4.vault())).unwrap();
        let cred: &keyring_core::mock::Cred = e4.as_any().downcast_ref().unwrap();
        cred.set_error(keyring_core::Error::Invalid(
            "mock error".to_owned(),
            "injected".to_owned(),
        ));
        let s5 = s4
            .vault()
            .unlock_without_agent(Some(&|| Ok(source())))
            .unwrap();
        assert!(
            s5.warnings()
                .iter()
                .any(|w| w.contains("keychain could not be read")),
            "expected a skip warning, got {:?}",
            s5.warnings()
        );
        // a read failure must not delete the entry
        assert!(is_enabled(s4.vault()).unwrap());

        // valid keychain key works with no passphrase source at all
        let s6 = s4.vault().unlock_without_agent(None).unwrap();
        assert!(s6.warnings().is_empty(), "unexpected warnings: {:?}", s6.warnings());
        disable(s4.vault()).unwrap();
        disable(s1.vault()).unwrap();
        disable(other.vault()).unwrap();
    }
}
