//! `ks insert` — store or update a secret.

use std::process::ExitCode;

use ks::{Config, Result, Secret};

use crate::commands;
use crate::prompt;
use crate::terminal;

pub fn run(config: &Config, path: &str, multiline: bool, force: bool) -> Result<ExitCode> {
    let store = commands::open_store(config)?;

    if store.exists(path)
        && !force
        && !prompt::confirm(&format!("{path} already exists — overwrite?"), false)?
    {
        terminal::warn("Aborted");
        return Ok(ExitCode::SUCCESS);
    }

    let raw = if multiline {
        prompt::multiline(&format!(
            "Enter secret for {path} (first line is the value; end with EOF)"
        ))?
    } else {
        prompt::secret_value(&format!("Value for {path}"))?
    };

    store.set(path, &Secret::new(raw.as_str()))?;
    terminal::success(&format!("Stored {path}"));
    Ok(ExitCode::SUCCESS)
}
