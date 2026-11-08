//! Touch ID device unlock via `age-plugin-se`.
//!
//! The vault identity text is encrypted to a Secure Enclave key
//! (`age1se1…` recipient), so macOS itself confirms every use with Touch
//! ID or the system password. `touchid.identity` and `touchid.age` live in
//! the local state directory; enabling requires macOS with a Secure
//! Enclave.

use std::path::PathBuf;
use std::str::FromStr as _;
use std::sync::{Arc, Mutex};

use age::secrecy::{ExposeSecret as _, SecretString};
use zeroize::Zeroizing;

use crate::unlock::Probe;
use crate::{Error, LocalKey, Result, Vault};

/// The plugin's short name (`age-plugin-se`, `age1se1…` recipients).
const PLUGIN: &str = "se";
/// `age-plugin-se keygen` output (the `AGE-PLUGIN-SE-…` identity file).
const IDENTITY_FILE: &str = "touchid.identity";
/// The vault key encrypted to the `age1se1…` recipient.
const BLOB_FILE: &str = "touchid.age";

/// Plugin callbacks: display messages are collected for the caller (the
/// library never prints); confirmations and input requests always return
/// `None` — the SE plugin uses the macOS system dialog itself.
#[derive(Clone, Default)]
struct PluginCallbacks {
    messages: Arc<Mutex<Vec<String>>>,
}

impl age::Callbacks for PluginCallbacks {
    fn display_message(&self, message: &str) {
        if let Ok(mut m) = self.messages.lock() {
            m.push(message.to_owned());
        }
    }

    fn confirm(&self, _: &str, _: &str, _: Option<&str>) -> Option<bool> {
        None
    }

    fn request_public_string(&self, _: &str) -> Option<String> {
        None
    }

    fn request_passphrase(&self, _: &str) -> Option<SecretString> {
        None
    }
}

fn identity_path(vault: &Vault) -> Result<PathBuf> {
    Ok(crate::local::state_dir(vault)?.join(IDENTITY_FILE))
}

fn blob_path(vault: &Vault) -> Result<PathBuf> {
    Ok(crate::local::state_dir(vault)?.join(BLOB_FILE))
}

/// Whether Touch ID is set up for this vault (both files present).
///
/// # Errors
///
/// [`Error::Unavailable`]/[`Error::Io`] when the state directory cannot be
/// resolved.
pub fn is_enabled(vault: &Vault) -> Result<bool> {
    Ok(identity_path(vault)?.exists() && blob_path(vault)?.exists())
}

/// Parses every `AGE-PLUGIN-SE-…` identity line from `touchid.identity`.
fn parse_identities(text: &str) -> Result<Vec<age::plugin::Identity>> {
    let mut identities = Vec::new();
    for line in text.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let id = age::plugin::Identity::from_str(line.trim()).map_err(|_| Error::Corrupt {
            path: IDENTITY_FILE.into(),
            reason: "bad plugin identity line",
        })?;
        identities.push(id);
    }
    if identities.is_empty() {
        return Err(Error::Corrupt {
            path: IDENTITY_FILE.into(),
            reason: "no plugin identities",
        });
    }
    Ok(identities)
}

/// Decrypts `touchid.age` with the `AGE-PLUGIN-SE-…` identities (runs the
/// plugin; the OS decides how to authenticate — Touch ID or password).
fn decrypt_blob(vault: &Vault) -> Result<Zeroizing<Vec<u8>>> {
    let identity_text = std::fs::read_to_string(identity_path(vault)?).map_err(|e| Error::Io {
        path: identity_path(vault).unwrap_or_default(),
        source: e,
    })?;
    let identities = parse_identities(&identity_text)?;
    let blob = std::fs::read(blob_path(vault)?).map_err(|e| Error::Io {
        path: blob_path(vault).unwrap_or_default(),
        source: e,
    })?;
    let plugin =
        age::plugin::IdentityPluginV1::new(PLUGIN, &identities, PluginCallbacks::default())
            .map_err(|e| Error::Unavailable(format!("age-plugin-se: {e}")))?;
    let decryptor = age::Decryptor::new_buffered(&blob[..]).map_err(|_| Error::Corrupt {
        path: BLOB_FILE.into(),
        reason: "not an age file",
    })?;
    let identity: &dyn age::Identity = &plugin;
    let mut reader = decryptor
        .decrypt(core::iter::once(identity))
        .map_err(|e| Error::Unavailable(format!("Touch ID decrypt failed: {e}")))?;
    let mut plaintext = Zeroizing::new(Vec::new());
    std::io::Read::read_to_end(&mut reader, &mut plaintext).map_err(|e| Error::Io {
        path: BLOB_FILE.into(),
        source: e,
    })?;
    Ok(plaintext)
}

/// Encrypts `plaintext` to `age1se1…` (spawns the plugin for the recipient
/// stanza; no authentication needed).
#[cfg(target_os = "macos")]
fn seal_to_se(recipient: &age::plugin::Recipient, plaintext: &[u8]) -> Result<Vec<u8>> {
    let plugin =
        age::plugin::RecipientPluginV1::new(PLUGIN, core::slice::from_ref(recipient), &[], PluginCallbacks::default())
            .map_err(|e| Error::Unavailable(format!("age-plugin-se: {e}")))?;
    let as_recipient: &dyn age::Recipient = &plugin;
    let encryptor = age::Encryptor::with_recipients(core::iter::once(as_recipient))
        .map_err(|_| Error::Internal("age encryption setup failed"))?;
    let mut out = Vec::new();
    let mut writer = encryptor.wrap_output(&mut out).map_err(|e| Error::Io {
        path: BLOB_FILE.into(),
        source: e,
    })?;
    std::io::Write::write_all(&mut writer, plaintext)
        .and_then(|()| writer.finish().map(|_| ()))
        .map_err(|e| Error::Io { path: BLOB_FILE.into(), source: e })?;
    Ok(out)
}

/// Stores the vault key behind Touch ID. `session` must hold the local key
/// (a passphrase/keychain unlock, never the agent). On any failure the
/// half-written files are removed.
///
/// # Errors
///
/// [`Error::Unavailable`] when `age-plugin-se` is missing or the verify
/// decrypt fails; [`Error::InvalidInput`] without a local key.
#[cfg(target_os = "macos")]
pub fn enable(session: &crate::Session) -> Result<()> {
    let Some(text) = session.agent_identity() else {
        return Err(Error::InvalidInput(
            "enabling Touch ID needs the master passphrase or the OS keychain".into(),
        ));
    };
    if which::which("age-plugin-se").is_err() {
        return Err(Error::Unavailable(
            "age-plugin-se not found; install it with `brew install age-plugin-se`".into(),
        ));
    }
    let dir = crate::local::state_dir(session.vault())?;
    let tmp = dir.join(".touchid.identity.tmp");
    let result = enable_inner(session, &dir, &tmp, &text);
    if result.is_err() {
        drop(std::fs::remove_file(&tmp));
        drop(std::fs::remove_file(dir.join(IDENTITY_FILE)));
        drop(std::fs::remove_file(dir.join(BLOB_FILE)));
    }
    result
}

#[cfg(target_os = "macos")]
fn enable_inner(
    session: &crate::Session,
    dir: &std::path::Path,
    tmp: &std::path::Path,
    text: &SecretString,
) -> Result<()> {
    let out = std::process::Command::new("age-plugin-se")
        .args(["keygen", "--access-control=any-biometry-or-passcode", "-o"])
        .arg(tmp)
        .output()
        .map_err(|e| Error::Unavailable(format!("cannot run age-plugin-se: {e}")))?;
    if !out.status.success() {
        return Err(Error::Unavailable(format!(
            "age-plugin-se keygen failed: {}",
            String::from_utf8_lossy(&out.stderr)
        )));
    }
    let identity_text = std::fs::read_to_string(tmp).map_err(|e| Error::Io {
        path: tmp.to_path_buf(),
        source: e,
    })?;
    let recipient = se_recipient(&identity_text)?;
    let encrypted = seal_to_se(&recipient, text.expose_secret().as_bytes())?;
    session
        .vault()
        .atomic_write(&dir.join(BLOB_FILE), &encrypted)?;
    std::fs::rename(tmp, dir.join(IDENTITY_FILE)).map_err(|e| Error::Io {
        path: tmp.to_path_buf(),
        source: e,
    })?;
    // Verify once: the plugin must hand back exactly the vault key text.
    match decrypt_blob(session.vault()) {
        Ok(plain) if *plain == *text.expose_secret().as_bytes() => Ok(()),
        Ok(_) => Err(Error::Internal("Touch ID verify decrypt returned a different key")),
        Err(e) => Err(e),
    }
}

/// Non-macOS builds have no Secure Enclave.
///
/// # Errors
///
/// Always [`Error::Unavailable`].
#[cfg(not(target_os = "macos"))]
pub fn enable(_session: &crate::Session) -> Result<()> {
    Err(Error::Unavailable(
        "Touch ID requires macOS with a Secure Enclave".into(),
    ))
}

/// Extracts the `age1se1…` recipient from a `# public key:` / `# recipient:`
/// comment in the keygen output.
#[cfg(target_os = "macos")]
fn se_recipient(identity_text: &str) -> Result<age::plugin::Recipient> {
    for line in identity_text.lines() {
        let Some(comment) = line.strip_prefix('#') else {
            continue;
        };
        let comment = comment.trim();
        if comment.contains("age1se1")
            && let Some(pos) = comment.find("age1se1")
        {
            return age::plugin::Recipient::from_str(&comment[pos..])
                .map_err(|_| Error::Internal("age-plugin-se produced a bad recipient"));
        }
    }
    Err(Error::Internal("age-plugin-se produced no recipient"))
}

/// Deletes the two Touch ID files; `true` if any existed.
///
/// # Errors
///
/// [`Error::Unavailable`]/[`Error::Io`] on filesystem failures.
pub fn disable(vault: &Vault) -> Result<bool> {
    let a = identity_path(vault)?;
    let b = blob_path(vault)?;
    let existed = a.exists() || b.exists();
    drop(std::fs::remove_file(&a));
    drop(std::fs::remove_file(&b));
    Ok(existed)
}

/// Unlock-chain probe: decrypts `touchid.age` and yields the key for the
/// check-file verification. Every failure is a soft skip — cancellation,
/// a missing plugin and read errors are never treated as stale.
#[cfg(not(target_os = "macos"))]
pub(crate) fn probe(_vault: &Vault) -> Probe {
    Probe::Absent
}

/// See the non-macOS doc comment.
#[cfg(target_os = "macos")]
pub(crate) fn probe(vault: &Vault) -> Probe {
    match probe_macos(vault) {
        Ok(p) => p,
        Err(e) => Probe::Skip(format!("Touch ID could not be used: {e}")),
    }
}

#[cfg(target_os = "macos")]
fn probe_macos(vault: &Vault) -> Result<Probe> {
    if !is_enabled(vault)? {
        return Ok(Probe::Absent);
    }
    let bytes = match decrypt_blob(vault) {
        Ok(b) => b,
        Err(e) => return Ok(Probe::Skip(format!("Touch ID unlock failed or was cancelled: {e}"))),
    };
    let Ok(text) = String::from_utf8(bytes.to_vec()) else {
        return Ok(Probe::Skip("Touch ID vault key is not UTF-8".to_owned()));
    };
    Ok(LocalKey::from_identity_text(&SecretString::from(text)).map_or_else(
        |_| Probe::Skip("Touch ID vault key is corrupt".to_owned()),
        Probe::Key,
    ))
}
