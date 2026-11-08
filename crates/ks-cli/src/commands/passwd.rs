//! `ks passwd` — change the master passphrase (not a revocation measure).

use super::{Cli, CliResult, Out, autocommit, err, prompt, vault};

/// Unlocks, takes the new passphrase, re-seals the identity file.
pub(crate) fn run(cli: &Cli, out: &Out) -> CliResult<i32> {
    let v = vault(cli)?;
    let source = prompt::passphrase_source(cli, out);
    let session = v.unlock_without_agent(Some(&source)).map_err(|e| err(&e))?;
    for w in session.warnings() {
        out.warn(w.clone());
    }
    let new = prompt::new_passphrase(out)?;
    let warnings = ks::check_new_passphrase(&new).map_err(|e| err(&e))?;
    session.change_passphrase(&new).map_err(|e| err(&e))?;
    let root = session.vault().root();
    autocommit(
        cli,
        out,
        &[root.join(".identity.age"), root.join(".check.age")],
        "ks: passwd",
    );
    for w in warnings {
        out.warn(w);
    }
    Ok(out.ok_value("passphrase changed", serde_json::json!({ "changed": true })))
}
