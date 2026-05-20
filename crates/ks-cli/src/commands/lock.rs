//! `ks lock` --clear the cached session identity.

use std::process::ExitCode;

use ks::{Config, Result, agent};

use crate::terminal;

pub fn run(config: &Config) -> Result<ExitCode> {
    agent::clear(&config.store_dir)?;
    terminal::success("Session cleared");
    Ok(ExitCode::SUCCESS)
}
