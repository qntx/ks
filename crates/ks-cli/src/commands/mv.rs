//! `ks mv` — rename a secret. Re-binds the envelope to the new path, so it
//! decrypts and re-encrypts and therefore unlocks the identity.

use std::process::ExitCode;

use ks::{Config, Result};

use crate::commands;
use crate::terminal;

pub fn run(config: &Config, from: &str, to: &str) -> Result<ExitCode> {
    let store = commands::open_store(config)?;
    let identity = commands::unlock(config)?;
    store.rename(from, to, &identity)?;
    if crate::output::is_json() {
        crate::output::emit(&serde_json::json!({ "from": from, "to": to, "moved": true }));
    } else {
        terminal::success(&format!("Moved {from} → {to}"));
    }
    Ok(ExitCode::SUCCESS)
}
