//! `ks keychain enable|disable` — OS keychain device unlock.

use super::{Cli, CliResult, Out, err, prompt, vault};

/// `enable` needs the vault key itself (passphrase/Touch-ID chain), writes
/// it to the OS credential store and reads it back.
pub(crate) fn run(cli: &Cli, out: &Out, enable: bool) -> CliResult<i32> {
    let v = vault(cli)?;
    if !enable {
        let was = ks::keychain::disable(&v).map_err(|e| err(&e))?;
        return Ok(out.ok_value(
            "keychain entry removed",
            serde_json::json!({ "disabled": true, "was_enabled": was }),
        ));
    }
    let source = prompt::passphrase_source(cli, out);
    let session = v.unlock_without_agent(Some(&source)).map_err(|e| err(&e))?;
    for w in session.warnings() {
        out.warn(w.clone());
    }
    for w in ks::keychain::enable(&session).map_err(|e| err(&e))? {
        out.warn(w);
    }
    Ok(out.ok_value(
        "keychain entry stored",
        serde_json::json!({ "enabled": true }),
    ))
}
