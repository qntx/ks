//! `ks get` — print or copy a secret value.

use std::process::ExitCode;

use ks::{Config, Error, Result};

use crate::clipboard;
use crate::commands;
use crate::terminal;

pub fn run(config: Config, path: &str, copy: bool, field: Option<&str>) -> Result<ExitCode> {
    let clear_secs = config.tunables.clipboard_clear_secs;
    let store = commands::open_store(config)?;
    let secret = store.get(path)?;

    let value = match field {
        None => secret.value.to_string(),
        Some(name) => secret
            .field(name)
            .ok_or_else(|| Error::SecretNotFound(format!("{path}#{name}")))?
            .to_owned(),
    };

    if copy {
        let secs = clipboard::copy_with_autoclear(&value, clear_secs)?;
        let what = field.map_or_else(|| path.to_owned(), |f| format!("{path}#{f}"));
        terminal::info(&format!("Copied {what} to clipboard (clears in {secs}s)"));
    } else {
        println!("{value}");
    }
    Ok(ExitCode::SUCCESS)
}
