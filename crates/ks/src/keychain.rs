//! OS keychain device unlock (feature `keychain`).
//!
//! The vault identity text lives in the platform credential store under
//! service `ks`, account `<vault id>`. Supported on macOS (legacy keychain
//! store, works for unsigned CLI binaries) and Windows (native credential
//! store); other platforms report the factor unavailable — Linux's Secret
//! Service would expose the key to every process of the user for the
//! whole desktop session, a strictly worse window than the session agent.
//!
//! The platform default store is installed once in this library; tests
//! install `keyring_core::mock::Store` instead.

#[cfg(any(target_os = "macos", windows))]
use std::sync::Arc;

#[cfg(any(target_os = "macos", windows))]
use age::secrecy::{ExposeSecret as _, SecretString};
#[cfg(any(target_os = "macos", windows))]
use zeroize::Zeroizing;

#[cfg(any(target_os = "macos", windows))]
use crate::LocalKey;
use crate::unlock::Probe;
use crate::{Error, Result, Session, Vault};

/// Keychain service name shared by all vaults; the account is the vault id.
#[cfg(any(target_os = "macos", windows))]
const SERVICE: &str = "ks";

/// The unavailable error for unsupported platforms.
#[cfg(not(any(target_os = "macos", windows)))]
const UNSUPPORTED: &str = "the OS keychain factor is available on macOS and Windows; on Linux use `ks unlock` for a session";

#[cfg(any(target_os = "macos", windows))]
fn entry(vault: &Vault) -> Result<keyring_core::Entry> {
    ensure_store()?;
    keyring_core::Entry::new(SERVICE, &crate::local::id(vault))
        .map_err(|e| Error::Unavailable(format!("cannot open the OS keychain: {e}")))
}

/// Installs the platform default store once (first use wins; a test may
/// install the mock store beforehand).
#[cfg(any(target_os = "macos", windows))]
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

/// Whether a keychain entry exists for this vault.
///
/// # Errors
///
/// [`Error::Unavailable`] when the store itself cannot be reached.
#[cfg(any(target_os = "macos", windows))]
pub fn is_enabled(vault: &Vault) -> Result<bool> {
    match entry(vault)?.get_secret() {
        Ok(_) => Ok(true),
        Err(keyring_core::Error::NoEntry) => Ok(false),
        Err(e) => Err(Error::Unavailable(format!("keychain read failed: {e}"))),
    }
}

/// `false` on platforms without a supported store.
///
/// # Errors
///
/// [`Error::Unavailable`] on platforms without a supported store.
#[cfg(not(any(target_os = "macos", windows)))]
#[allow(
    clippy::unnecessary_wraps,
    reason = "the supported branch can fail; Result is the cross-platform contract"
)]
pub const fn is_enabled(_vault: &Vault) -> Result<bool> {
    Ok(false)
}

/// Stores the vault key in the OS keychain. `session` must hold the local
/// key (a passphrase/Touch-ID unlock, never the agent).
///
/// # Errors
///
/// [`Error::InvalidInput`] without a local key; [`Error::Unavailable`] when
/// the store cannot be reached or the read-back check fails.
#[cfg(any(target_os = "macos", windows))]
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
    Ok(Vec::new())
}

/// [`Error::Unavailable`] on platforms without a supported store.
///
/// # Errors
///
/// Always [`Error::Unavailable`] — no supported store exists.
#[cfg(not(any(target_os = "macos", windows)))]
pub fn enable(_session: &Session) -> Result<Vec<String>> {
    Err(Error::Unavailable(UNSUPPORTED.into()))
}

/// Deletes the keychain entry; `true` if one existed.
///
/// # Errors
///
/// [`Error::Unavailable`] when the store cannot be reached.
#[cfg(any(target_os = "macos", windows))]
pub fn disable(vault: &Vault) -> Result<bool> {
    match entry(vault)?.delete_credential() {
        Ok(()) => Ok(true),
        Err(keyring_core::Error::NoEntry) => Ok(false),
        Err(e) => Err(Error::Unavailable(format!("keychain delete failed: {e}"))),
    }
}

/// `false` on platforms without a supported store.
///
/// # Errors
///
/// [`Error::Unavailable`] on platforms without a supported store.
#[cfg(not(any(target_os = "macos", windows)))]
#[allow(
    clippy::unnecessary_wraps,
    reason = "the supported branch can fail; Result is the cross-platform contract"
)]
pub const fn disable(_vault: &Vault) -> Result<bool> {
    Ok(false)
}

/// Unlock-chain probe: reads the keychain entry and yields the key for the
/// check-file verification. Every failure is a soft skip — access denied,
/// read errors and an unavailable store are never treated as stale.
#[cfg(any(target_os = "macos", windows))]
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

/// `Absent` on platforms without a supported store.
#[cfg(not(any(target_os = "macos", windows)))]
pub(crate) const fn probe(_vault: &Vault) -> Probe {
    Probe::Absent
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::panic,
        reason = "test harness: panicking fixtures are intentional"
    )]

    use age::secrecy::SecretString;

    use super::*;

    const PASS: &str = "correct horse battery staple fixture";

    #[cfg(any(target_os = "macos", windows))]
    mod supported {
        use super::*;

        fn vault() -> (tempfile::TempDir, Session) {
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path().join("v");
            let session = Vault::create(&root, &SecretString::from(PASS)).unwrap();
            (dir, session)
        }

        fn source() -> SecretString {
            SecretString::from(PASS)
        }

        fn mock() -> Arc<keyring_core::mock::Store> {
            let store = keyring_core::mock::Store::new().unwrap();
            let ret = Arc::clone(&store);
            let as_store: Arc<keyring_core::CredentialStore> = store;
            keyring_core::set_default_store(as_store);
            ret
        }

        /// UNLOCK-ORDER: an enabled device source is tried before the
        /// passphrase — unlocking must not consult the source at all.
        #[test]
        fn unlock_order_device_before_passphrase() {
            mock();
            let (_d, s) = vault();
            drop(enable(&s).unwrap());
            let s2 = s
                .vault()
                .unlock_without_agent(Some(&|| panic!("passphrase source must not be consulted")))
                .unwrap();
            assert!(s2.warnings().is_empty(), "{:?}", s2.warnings());
        }

        /// All keychain tests share one global store, so they run in one fn.
        #[test]
        fn device_stale_keychain_roundtrip_and_errors() {
            mock();
            let (_d1, s1) = vault();
            drop(enable(&s1).unwrap());
            assert!(is_enabled(s1.vault()).unwrap());
            let s2 = s1.vault().unlock_without_agent(None).unwrap();
            s2.get(&crate::Name::parse("x").unwrap()).unwrap_err();
            assert!(disable(s1.vault()).unwrap());
            assert!(!is_enabled(s1.vault()).unwrap());

            let (_d2, other) = vault();
            let other_text = other.agent_identity().unwrap();
            let e = keyring_core::Entry::new(SERVICE, &crate::local::id(s1.vault())).unwrap();
            e.set_secret(other_text.expose_secret().as_bytes()).unwrap();
            let s3 = s1
                .vault()
                .unlock_without_agent(Some(&|| Ok(source())))
                .unwrap();
            assert!(
                s3.warnings().iter().any(|w| w.contains("keychain")),
                "expected a stale-keychain warning, got {:?}",
                s3.warnings()
            );
            assert!(!is_enabled(s1.vault()).unwrap());

            let (_d3, s4) = vault();
            drop(enable(&s4).unwrap());
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
            assert!(is_enabled(s4.vault()).unwrap());

            let s6 = s4.vault().unlock_without_agent(None).unwrap();
            assert!(
                s6.warnings().is_empty(),
                "unexpected warnings: {:?}",
                s6.warnings()
            );
            disable(s4.vault()).unwrap();
            disable(s1.vault()).unwrap();
            disable(other.vault()).unwrap();
        }
    }

    /// Unsupported platforms report the factor unavailable, never enabled.
    #[cfg(not(any(target_os = "macos", windows)))]
    #[test]
    fn unsupported_platform() {
        let dir = tempfile::tempdir().unwrap();
        let session = Vault::create(&dir.path().join("v"), &SecretString::from(PASS)).unwrap();
        let err = enable(&session).unwrap_err();
        assert!(matches!(err, Error::Unavailable(_)), "{err}");
        assert!(err.to_string().contains("macOS and Windows"), "{err}");
        assert!(!is_enabled(session.vault()).unwrap());
        assert!(matches!(probe(session.vault()), Probe::Absent));
        assert!(!disable(session.vault()).unwrap());
    }
}
