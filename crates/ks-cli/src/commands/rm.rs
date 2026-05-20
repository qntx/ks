//! `ks rm` --delete a secret.

use std::process::ExitCode;

use ks::{Config, Result};

use crate::commands;
use crate::prompt;
use crate::terminal;

pub fn run(config: &Config, path: &str, force: bool) -> Result<ExitCode> {
    let store = commands::open_store(config)?;

    if !force {
        let ok = prompt::confirm(&format!("Delete {path}?"), false)?;
        if !ok {
            terminal::warn("Aborted");
            return Ok(ExitCode::SUCCESS);
        }
    }

    store.delete(path)?;
    terminal::success(&format!("Deleted {path}"));
    Ok(ExitCode::SUCCESS)
}
