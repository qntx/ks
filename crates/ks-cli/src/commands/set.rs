//! `ks set` — store or update a secret.

use std::process::ExitCode;

use ks::{Config, Result, Secret};

use crate::commands;
use crate::prompt;
use crate::terminal;

pub fn run(
    config: Config,
    path: &str,
    note: Option<&str>,
    force: bool,
    totp: bool,
) -> Result<ExitCode> {
    let store = commands::open_store(config)?;

    if store.exists(path) && !force {
        let ok = prompt::confirm(&format!("{path} already exists — overwrite?"), false)?;
        if !ok {
            terminal::warn("Aborted");
            return Ok(ExitCode::SUCCESS);
        }
    }

    let raw = prompt::secret_value(&format!("Value for {path}"))?;
    let mut secret = Secret::new(raw.as_str());
    if let Some(n) = note {
        secret = secret.with_note(n);
    }
    if totp {
        secret = secret.into_totp();
    }

    store.set(path, &secret)?;
    terminal::success(&format!("Stored {path}"));
    Ok(ExitCode::SUCCESS)
}
