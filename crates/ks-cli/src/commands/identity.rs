//! `ks identity` — print this device's public recipient.

use std::process::ExitCode;

use ks::{Config, Result};

use crate::commands;

pub fn run(config: &Config) -> Result<ExitCode> {
    let identity = commands::unlock(config)?;
    println!("{}", identity.to_public());
    Ok(ExitCode::SUCCESS)
}
