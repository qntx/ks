//! `ks rm <name> [--force]` — delete an entry.

use super::{Cli, CliResult, ErrorCode, Out, err, name, prompt, unlock};

/// Removes the entry after confirmation (or `--force`).
pub(crate) fn run(cli: &Cli, out: &Out, n: &str, force: bool) -> CliResult<i32> {
    let session = unlock(cli, out)?;
    let name = name(n)?;
    if !force {
        if out.interactive {
            prompt::confirm(out, &format!("remove {name}?"))?;
        } else {
            return Err((
                ErrorCode::Usage,
                "removing an entry requires --force non-interactively".to_owned(),
            ));
        }
    }
    session.remove(&name).map_err(|e| err(&e))?;
    Ok(out.ok_value(
        &format!("removed {name}"),
        serde_json::json!({ "name": name.to_string(), "removed": true }),
    ))
}
