//! `ks edit` — edit a secret in `$EDITOR`.

use std::path::Path;
use std::process::{Command as Proc, ExitCode};
use std::time::{SystemTime, UNIX_EPOCH};

use ks::{Config, Error, Result, Secret};
use zeroize::Zeroizing;

use crate::commands;
use crate::terminal;

pub fn run(config: &Config, path: &str) -> Result<ExitCode> {
    let store = commands::open_store(config)?;

    // Editing an existing secret needs the plaintext, so unlock; creating a new
    // one only writes, so stay locked (consistent with `insert`).
    let original = if store.exists(path) {
        let identity = commands::unlock(config)?;
        Zeroizing::new(store.get(path, &identity)?.expose().to_owned())
    } else {
        Zeroizing::new(String::new())
    };

    let edited = edit_in_editor(&original)?;
    if *edited == *original {
        terminal::warn("No changes");
        return Ok(ExitCode::SUCCESS);
    }

    store.set(path, &Secret::new(edited.as_str()))?;
    terminal::success(&format!("Updated {path}"));
    Ok(ExitCode::SUCCESS)
}

/// Round-trips `initial` through `$EDITOR` via a short-lived temp file
/// (mode `0o600` on Unix), removed regardless of the editor's outcome.
fn edit_in_editor(initial: &str) -> Result<Zeroizing<String>> {
    let tmp = temp_path();
    std::fs::write(&tmp, initial)?;
    set_owner_only(&tmp);
    let outcome = run_editor(&tmp);
    if let Err(_e) = std::fs::remove_file(&tmp) {}
    outcome
}

fn run_editor(tmp: &Path) -> Result<Zeroizing<String>> {
    let (program, args) = editor();
    let status = Proc::new(&program)
        .args(&args)
        .arg(tmp)
        .status()
        .map_err(Error::Io)?;
    if !status.success() {
        return Err(Error::Io(std::io::Error::other(format!(
            "editor `{program}` exited without saving"
        ))));
    }
    Ok(Zeroizing::new(std::fs::read_to_string(tmp)?))
}

/// Resolves the editor invocation from `$EDITOR`/`$VISUAL` (falling back to a
/// platform default), split on whitespace into program plus arguments.
///
/// Splitting on whitespace means an editor *path* that itself contains spaces
/// is unsupported; point `$EDITOR` at a wrapper script in that case.
fn editor() -> (String, Vec<String>) {
    let raw = std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .unwrap_or_else(|_| default_editor().to_owned());
    let mut parts = raw.split_whitespace().map(str::to_owned);
    let program = parts.next().unwrap_or_else(|| default_editor().to_owned());
    (program, parts.collect())
}

const fn default_editor() -> &'static str {
    if cfg!(windows) { "notepad" } else { "vi" }
}

fn temp_path() -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    std::env::temp_dir().join(format!("ks-edit-{}-{nanos}.txt", std::process::id()))
}

#[cfg(unix)]
fn set_owner_only(path: &Path) {
    use std::os::unix::fs::PermissionsExt as _;
    if let Ok(meta) = std::fs::metadata(path) {
        let mut perms = meta.permissions();
        perms.set_mode(0o600);
        if let Err(_e) = std::fs::set_permissions(path, perms) {}
    }
}

#[cfg(not(unix))]
const fn set_owner_only(_path: &Path) {}
