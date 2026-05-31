//! `ks insert` — store or update a secret.

use std::process::ExitCode;

use ks::{Config, Error, Result, Secret, SecretKind};

use crate::commands;
use crate::prompt;
use crate::terminal;

pub fn run(
    config: &Config,
    path: &str,
    multiline: bool,
    force: bool,
    binary: bool,
) -> Result<ExitCode> {
    let store = commands::open_store(config)?;
    let json = crate::output::is_json();

    if store.exists(path) && !force {
        if json {
            return Err(Error::SecretExists(path.to_owned()));
        }
        if !prompt::confirm(&format!("{path} already exists — overwrite?"), false)? {
            terminal::warn("Aborted");
            return Ok(ExitCode::SUCCESS);
        }
    }

    let secret = if binary {
        Secret::from_bytes(prompt::stdin_bytes()?, SecretKind::Binary)
    } else if multiline {
        let raw = prompt::multiline(&format!(
            "Enter secret for {path} (first line is the value; end with EOF)"
        ))?;
        Secret::new(raw.as_str())
    } else {
        let raw = prompt::secret_value(&format!("Value for {path}"))?;
        Secret::new(raw.as_str())
    };

    store.set(path, &secret)?;
    if json {
        crate::output::emit(&serde_json::json!({ "path": path, "stored": true }));
    } else {
        terminal::success(&format!("Stored {path}"));
    }
    Ok(ExitCode::SUCCESS)
}
