//! `ks touchid enable|disable` — Secure Enclave device unlock.

use super::{Cli, CliResult, Out, err, prompt, vault};

/// `enable` needs the vault key itself (passphrase/keychain chain), runs
/// `age-plugin-se keygen` and stores `touchid.identity` + `touchid.age`.
pub(crate) fn run(cli: &Cli, out: &Out, enable: bool) -> CliResult<i32> {
    let v = vault(cli)?;
    if !enable {
        let was = ks::touchid::disable(&v).map_err(|e| err(&e))?;
        return Ok(out.ok_value(
            "Touch ID disabled",
            serde_json::json!({ "disabled": true, "was_enabled": was }),
        ));
    }
    let source = prompt::passphrase_source(cli, out);
    let session = v.unlock_without_agent(Some(&source)).map_err(|e| err(&e))?;
    for w in session.warnings() {
        out.warn(w.clone());
    }
    ks::touchid::enable(&session).map_err(|e| err(&e))?;
    Ok(out.ok_value("Touch ID enabled", serde_json::json!({ "enabled": true })))
}
