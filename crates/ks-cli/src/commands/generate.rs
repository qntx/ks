//! `ks gen` — generate a random secret and optionally store it.

use std::process::ExitCode;

use ks::pwgen::{self, Charset};
use ks::{Config, Error, Result, Secret};

use crate::clipboard;
use crate::commands;
use crate::prompt;
use crate::terminal;

pub fn run(
    config: &Config,
    path: Option<&str>,
    length: usize,
    charset: Charset,
    force: bool,
    copy: bool,
) -> Result<ExitCode> {
    let value =
        pwgen::generate(length, charset).map_err(|e| Error::InvalidArgument(e.to_owned()))?;
    let json = crate::output::is_json();

    if let Some(path) = path {
        let store = commands::open_store(config)?;
        if store.exists(path) && !force {
            if json {
                return Err(Error::SecretExists(path.to_owned()));
            }
            if !prompt::confirm(&format!("{path} already exists — overwrite?"), false)? {
                terminal::warn("Aborted");
                return Ok(ExitCode::SUCCESS);
            }
        }
        store.set(path, &Secret::new(value.as_str()))?;
        if !json {
            terminal::success(&format!("Stored generated secret at {path}"));
        }
    }

    if json {
        crate::output::emit(&serde_json::json!({
            "value": value.as_str(),
            "length": length,
            "charset": charset.name(),
            "stored": path,
        }));
        return Ok(ExitCode::SUCCESS);
    }

    if copy {
        let secs = clipboard::copy_with_autoclear(value.as_str(), commands::clip_secs())?;
        terminal::info(&format!("Copied to clipboard (clears in {secs}s)"));
    } else if path.is_none() {
        println!("{}", value.as_str());
    }
    Ok(ExitCode::SUCCESS)
}
