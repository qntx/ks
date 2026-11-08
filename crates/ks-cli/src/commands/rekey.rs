//! `ks rekey` — rotate the vault key (the only revocation measure).

use super::{Cli, CliResult, Out, autocommit, err, prompt, vault};

/// Unlocks with the master passphrase only (never the agent or a device
/// factor — a new passphrase must be set), rotates K, and auto-commits the
/// rewritten files as `ks: rekey`.
pub(crate) fn run(cli: &Cli, out: &Out) -> CliResult<i32> {
    let v = vault(cli)?;
    let source = prompt::passphrase_source(cli, out);
    let passphrase = source().map_err(|e| err(&e))?;
    // prompt::new_passphrase yields a CLI error pair; the callback needs a
    // library error, so the real error is stashed and re-raised after.
    let prompt_err = std::cell::RefCell::new(None);
    let result = v.rekey(&passphrase, &|| {
        prompt::new_passphrase(out).map_err(|e| {
            *prompt_err.borrow_mut() = Some(e);
            ks::Error::InvalidInput("new passphrase prompt failed".into())
        })
    });
    if let Some(e) = prompt_err.into_inner() {
        return Err(e);
    }
    let report = result.map_err(|e| err(&e))?;
    for w in &report.warnings {
        out.warn(w.clone());
    }
    autocommit(cli, out, &report.paths, "ks: rekey");
    out.warn(
        "key rotated: push with `ks sync`; on every other device unlock with \
         the new passphrase (stale Touch ID, keychain and agent state is \
         removed automatically — re-enable them); and revoke the lost \
         device's git credentials"
            .to_owned(),
    );
    Ok(out.ok_value(
        &format!(
            "key rotated{} ({} entries)",
            if report.resumed { " (resumed)" } else { "" },
            report.entries
        ),
        serde_json::json!({
            "rekeyed": true,
            "resumed": report.resumed,
            "entries": report.entries,
        }),
    ))
}
