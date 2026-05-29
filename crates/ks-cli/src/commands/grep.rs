//! `ks grep` — search secrets by path, and optionally by decrypted content.

use std::process::ExitCode;

use ks::{Config, Result};

use crate::commands;
use crate::terminal;

pub fn run(config: &Config, query: &str, values: bool) -> Result<ExitCode> {
    let store = commands::open_store(config)?;
    let identity = if values {
        Some(commands::unlock(config)?)
    } else {
        None
    };

    let hits = store.grep(query, identity.as_ref())?;
    if hits.is_empty() {
        terminal::warn("No matches");
        return Ok(ExitCode::SUCCESS);
    }
    for path in hits {
        println!("{path}");
    }
    Ok(ExitCode::SUCCESS)
}
