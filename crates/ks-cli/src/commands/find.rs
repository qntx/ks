//! `ks find` — fuzzy-search secrets by path (and optionally notes).

use std::process::ExitCode;

use ks::{Config, Result};

use crate::commands;
use crate::terminal;

pub fn run(config: Config, query: &str, include_notes: bool) -> Result<ExitCode> {
    let store = commands::open_store(config)?;
    let hits = store.find(query, include_notes)?;

    if hits.is_empty() {
        terminal::warn(&format!("No matches for `{query}`"));
        return Ok(ExitCode::SUCCESS);
    }
    for path in hits {
        println!("{path}");
    }
    Ok(ExitCode::SUCCESS)
}
