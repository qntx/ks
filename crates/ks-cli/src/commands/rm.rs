//! `ks rm` — remove a secret.

use std::process::ExitCode;

use ks::{Config, Error, Result};

use crate::commands;
use crate::prompt;
use crate::terminal;

pub fn run(config: &Config, path: &str, force: bool) -> Result<ExitCode> {
    let store = commands::open_store(config)?;
    let json = crate::output::is_json();
    if !store.exists(path) {
        return Err(Error::SecretNotFound(path.to_owned()));
    }
    if !force {
        if json {
            return Err(Error::InvalidArgument(format!(
                "refusing to remove `{path}` without --force in --json mode"
            )));
        }
        if !prompt::confirm(&format!("Delete {path}?"), false)? {
            terminal::warn("Aborted");
            return Ok(ExitCode::SUCCESS);
        }
    }
    store.delete(path)?;
    if json {
        crate::output::emit(&serde_json::json!({ "path": path, "removed": true }));
    } else {
        terminal::success(&format!("Removed {path}"));
    }
    Ok(ExitCode::SUCCESS)
}
