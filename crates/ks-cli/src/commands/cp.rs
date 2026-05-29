//! `ks cp` — copy a secret (ciphertext copy, no passphrase needed).

use std::process::ExitCode;

use ks::{Config, Result};

use crate::commands;
use crate::terminal;

pub fn run(config: &Config, from: &str, to: &str) -> Result<ExitCode> {
    commands::open_store(config)?.copy(from, to)?;
    terminal::success(&format!("Copied {from} → {to}"));
    Ok(ExitCode::SUCCESS)
}
