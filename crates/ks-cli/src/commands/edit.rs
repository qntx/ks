//! `ks edit <name>` — edit a text entry in an external editor.

use std::io::Write as _;

use ks::{Entry, ErrorCode, Kind};
use zeroize::Zeroizing;

use super::{Cli, CliResult, Out, err, fail, name, unlock};

/// Opens the entry in `$VISUAL`/`$EDITOR`/`vi` inside a private temp dir.
pub(crate) fn run(cli: &Cli, out: &Out, n: &str) -> CliResult<i32> {
    if !out.interactive {
        return fail(ErrorCode::Usage, "edit requires an interactive terminal");
    }
    let session = unlock(cli, out)?;
    let name = name(n)?;
    let existing = match session.get(&name) {
        Ok(e) => Some(e),
        Err(ks::Error::NotFound(_)) => None,
        Err(e) => return Err(err(&e)),
    };
    if let Some(e) = &existing
        && e.kind() == Kind::Binary
    {
        return fail(ErrorCode::Usage, "binary entries cannot be edited");
    }
    let current = existing
        .as_ref()
        .and_then(|e| e.as_text().map(|t| t.as_str().to_owned()))
        .unwrap_or_default();

    let dir = tempfile::Builder::new()
        .prefix("ks-edit-")
        .tempdir()
        .map_err(|e| {
            err(&ks::Error::Io {
                path: "tempdir".into(),
                source: e,
            })
        })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).ok();
    }
    let file = dir.path().join(name.as_str().replace('/', "_"));
    {
        let mut f = std::fs::File::create(&file).map_err(|e| {
            err(&ks::Error::Io {
                path: file.clone(),
                source: e,
            })
        })?;
        // tempfile::NamedTempFile would give 0600; File::create honors umask
        // inside a 0700 dir, enforce 0600 for parity with vault writes.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            f.set_permissions(std::fs::Permissions::from_mode(0o600))
                .ok();
        }
        f.write_all(current.as_bytes()).map_err(|e| {
            err(&ks::Error::Io {
                path: file.clone(),
                source: e,
            })
        })?;
    }

    let editor = std::env::var("VISUAL")
        .ok()
        .filter(|v| !v.is_empty())
        .or_else(|| std::env::var("EDITOR").ok().filter(|v| !v.is_empty()))
        .unwrap_or_else(|| "vi".to_owned());
    let mut parts = editor.split_whitespace();
    let program = parts.next().unwrap_or("vi");
    let status = std::process::Command::new(program)
        .args(parts)
        .arg(&file)
        .status()
        .map_err(|e| {
            (
                ErrorCode::Unavailable,
                format!("cannot start editor {program}: {e}"),
            )
        })?;
    if !status.success() {
        scrub(&dir);
        return fail(ErrorCode::Cancelled, "editor exited without saving");
    }
    let new_text = std::fs::read_to_string(&file).map_err(|e| {
        err(&ks::Error::Io {
            path: file.clone(),
            source: e,
        })
    })?;
    scrub(&dir);
    if new_text == current {
        return Ok(out.ok_value(
            "unchanged",
            serde_json::json!({ "name": name.to_string(), "stored": false }),
        ));
    }
    let entry = Entry::text(Zeroizing::new(new_text)).map_err(|e| err(&e))?;
    session.set(&name, &entry, true).map_err(|e| err(&e))?;
    Ok(out.ok_value(
        &format!("stored {name}"),
        serde_json::json!({ "name": name.to_string(), "stored": true }),
    ))
}

/// Overwrites the temp file with zeros and removes the dir.
fn scrub(dir: &tempfile::TempDir) {
    for entry in std::fs::read_dir(dir.path())
        .into_iter()
        .flatten()
        .flatten()
    {
        let p = entry.path();
        if p.is_file()
            && let Ok(len) = p.metadata().map(|m| m.len())
        {
            let zeros = vec![0u8; usize::try_from(len).unwrap_or(0)];
            // Best-effort scrub: the TempDir still deletes the file on drop.
            std::fs::write(&p, zeros).ok();
        }
    }
}
