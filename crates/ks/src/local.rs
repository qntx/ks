//! Per-vault local state shared by the agent, Touch ID and the keychain:
//! the vault id and the on-disk state directory. Nothing here syncs.

use std::path::PathBuf;

use sha2::Digest as _;

use crate::{Error, Result, Vault};

/// The vault id: hex of the first 8 bytes of SHA-256 over the canonical
/// vault root path bytes.
#[must_use]
pub fn id(vault: &Vault) -> String {
    let digest = sha2::Sha256::digest(vault.root().as_os_str().as_encoded_bytes());
    let mut short = [0u8; 8];
    if let Some(first) = digest.get(..8) {
        short.copy_from_slice(first);
    }
    crate::hex(short)
}

/// The shared base directory: `ProjectDirs::from("", "", "ks")`'s
/// `data_local_dir`.
///
/// # Errors
///
/// [`Error::Unavailable`] when no state directory can be resolved.
pub(crate) fn data_dir() -> Result<PathBuf> {
    directories::ProjectDirs::from("", "", "ks")
        .map(|p| p.data_local_dir().to_path_buf())
        .ok_or_else(|| Error::Unavailable("cannot resolve a state directory".into()))
}

/// `<data_local_dir>/<id>` — the per-vault local state directory.
///
/// Returns the path only and never creates it: read-side callers
/// (probes, `is_enabled`, `disable`) must not leave empty dirs behind.
/// An existing directory must be owned privately by the current user;
/// a missing one is simply absent state.
///
/// # Errors
///
/// [`Error::Unavailable`] on an unsafe directory; [`Error::Io`] on
/// filesystem failures.
pub fn state_dir(vault: &Vault) -> Result<PathBuf> {
    let dir = data_dir()?.join(id(vault));
    if dir.is_dir() {
        check_dir_perms(&dir)?;
    }
    Ok(dir)
}

/// `state_dir` plus creation with mode `0700` on unix — only for code
/// that actually writes device state (`touchid enable`).
///
/// # Errors
///
/// [`Error::Unavailable`] on an unsafe directory; [`Error::Io`] on
/// filesystem failures.
pub fn ensure_state_dir(vault: &Vault) -> Result<PathBuf> {
    let dir = data_dir()?.join(id(vault));
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt as _;
        builder.mode(0o700);
    }
    builder.create(&dir).map_err(|e| Error::Io {
        path: dir.clone(),
        source: e,
    })?;
    check_dir_perms(&dir)?;
    Ok(dir)
}

/// A unix state/endpoint directory must be owned by the current uid and
/// carry no group/other permission bits (`MetadataExt` + `rustix`, no unsafe).
#[cfg(unix)]
pub(crate) fn check_dir_perms(dir: &std::path::Path) -> Result<()> {
    use std::os::unix::fs::MetadataExt as _;
    let meta = std::fs::metadata(dir).map_err(|e| Error::Io {
        path: dir.to_path_buf(),
        source: e,
    })?;
    if meta.uid() != rustix::process::getuid().as_raw() || meta.mode() & 0o077 != 0 {
        return Err(Error::Unavailable(
            "local state directory is not owned privately by this user".into(),
        ));
    }
    Ok(())
}

/// Windows: ownership enforcement comes from NTFS ACLs on the user profile.
/// `Result` keeps the cross-platform signature; the unix branch can fail.
#[cfg(windows)]
#[allow(
    clippy::unnecessary_wraps,
    reason = "unix branch can fail; Result is the cross-platform contract"
)]
pub(crate) const fn check_dir_perms(_dir: &std::path::Path) -> Result<()> {
    Ok(())
}
