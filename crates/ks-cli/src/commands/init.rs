//! `ks init` — create a new vault.

use ks::Vault;

use super::*;

/// Creates the vault; the new passphrase comes from the interactive double
/// prompt or `KS_NEW_PASSPHRASE_FILE`.
pub fn run(cli: &Cli, out: &mut Out) -> CliResult<i32> {
    let path = cli.vault_path().map_err(|e| err(&e))?;
    let passphrase = prompt::new_passphrase(out)?;
    let warnings = ks::check_new_passphrase(&passphrase).map_err(|e| err(&e))?;
    let _session = Vault::create(&path, &passphrase).map_err(|e| err(&e))?;
    for w in warnings {
        out.warn(w);
    }
    Ok(out.ok_value(
        &format!("created vault at {}", path.display()),
        serde_json::json!({ "created": true, "path": path.display().to_string() }),
    ))
}
