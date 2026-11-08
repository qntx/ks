//! `ks passwd` — change the master passphrase (not a revocation measure).

use super::{Cli, CliResult, Out, err, prompt, unlock};

/// Unlocks, takes the new passphrase, re-seals the identity file.
pub(crate) fn run(cli: &Cli, out: &Out) -> CliResult<i32> {
    let session = unlock(cli, out)?;
    let new = prompt::new_passphrase(out)?;
    let warnings = ks::check_new_passphrase(&new).map_err(|e| err(&e))?;
    session.change_passphrase(&new).map_err(|e| err(&e))?;
    for w in warnings {
        out.warn(w);
    }
    Ok(out.ok_value("passphrase changed", serde_json::json!({ "changed": true })))
}
