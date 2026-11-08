//! `ks lock` — ask the session agent to exit.

use super::{Cli, CliResult, Out, err, vault};

/// Idempotent: reports whether an agent was running.
pub(crate) fn run(cli: &Cli, out: &Out) -> CliResult<i32> {
    let v = vault(cli)?;
    let was_running = ks::agent::lock(&v).map_err(|e| err(&e))?;
    Ok(out.ok_value(
        if was_running {
            "agent locked"
        } else {
            "no agent running"
        },
        serde_json::json!({ "locked": true, "was_running": was_running }),
    ))
}
