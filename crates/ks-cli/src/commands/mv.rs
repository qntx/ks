//! `ks mv <from> <to> [--force]` — rename an entry.

use super::{autocommit, Cli, CliResult, Out, err, name, prompt, unlock};

/// Re-encrypts under the new name, then deletes the old file.
pub(crate) fn run(cli: &Cli, out: &Out, from: &str, to: &str, force: bool) -> CliResult<i32> {
    let session = unlock(cli, out)?;
    let from = name(from)?;
    let to = name(to)?;
    let dest_exists = to.path_in(session.vault().root()).exists();
    if dest_exists && !force {
        if out.interactive {
            prompt::confirm(out, &format!("overwrite {to}?"))?;
        } else {
            return Err(err(&ks::Error::Exists(to.to_string())));
        }
    }
    let root = session.vault().root();
    session.rename(&from, &to, true).map_err(|e| err(&e))?;
    autocommit(
        cli,
        out,
        &[from.path_in(root), to.path_in(root)],
        &format!("ks: mv {from} {to}"),
    );
    Ok(out.ok_value(
        &format!("renamed {from} -> {to}"),
        serde_json::json!({ "from": from.to_string(), "to": to.to_string(), "renamed": true }),
    ))
}
