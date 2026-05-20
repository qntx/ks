//! `ks mv` -- rename / move a secret.

use std::process::ExitCode;

use ks::{Config, Result};

use crate::commands;
use crate::terminal;

pub fn run(config: &Config, from: &str, to: &str) -> Result<ExitCode> {
    let store = commands::open_store(config)?;
    store.rename(from, to)?;
    terminal::success(&format!("Renamed {from} -> {to}"));
    Ok(ExitCode::SUCCESS)
}
