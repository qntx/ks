//! `ks ls` --list secrets as a tree.

use std::process::ExitCode;

use ks::{Config, Result};

use crate::commands;
use crate::terminal;

pub fn run(config: &Config, prefix: &str) -> Result<ExitCode> {
    let store = commands::open_store(config)?;
    let paths = store.list(prefix)?;
    let view: Vec<&str> = paths.iter().map(String::as_str).collect();
    terminal::tree(&view);
    Ok(ExitCode::SUCCESS)
}
