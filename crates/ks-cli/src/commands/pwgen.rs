//! `ks gen` — generate a random secret.

use std::process::ExitCode;
use std::str::FromStr as _;

use ks::pwgen::{self, Charset};
use ks::{Config, Error, Result, Secret};

use crate::clipboard;
use crate::commands;
use crate::terminal;

pub fn run(
    config: Config,
    path: Option<&str>,
    length: usize,
    charset: &str,
    force: bool,
    copy: bool,
) -> Result<ExitCode> {
    let cs = Charset::from_str(charset).map_err(Error::InvalidPath)?;
    let value = pwgen::generate(length, cs).map_err(|e| Error::InvalidPath(e.into()))?;
    let clear_secs = config.tunables.clipboard_clear_secs;

    if let Some(target) = path {
        let store = commands::open_store(config)?;
        if store.exists(target) && !force {
            return Err(Error::SecretExists(target.to_owned()));
        }
        store.set(target, &Secret::new(value.as_str()))?;
        terminal::success(&format!("Generated and stored {target} ({length} chars)"));
        if copy {
            let secs = clipboard::copy_with_autoclear(value.as_str(), clear_secs)?;
            terminal::info(&format!("Copied to clipboard (clears in {secs}s)"));
        }
    } else if copy {
        let secs = clipboard::copy_with_autoclear(value.as_str(), clear_secs)?;
        terminal::info(&format!(
            "Copied generated value to clipboard (clears in {secs}s)"
        ));
    } else {
        println!("{}", value.as_str());
    }
    Ok(ExitCode::SUCCESS)
}
