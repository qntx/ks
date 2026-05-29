//! `ks passwd` — change the identity passphrase.

use std::process::ExitCode;

use ks::{Config, Result, crypto};

use crate::prompt;
use crate::terminal;

pub fn run(config: &Config) -> Result<ExitCode> {
    let current = prompt::passphrase("Current passphrase")?;
    let new = prompt::new_passphrase("New passphrase")?;
    crypto::change_passphrase(&config.identity_path, current, new)?;
    terminal::success("Passphrase changed");
    Ok(ExitCode::SUCCESS)
}
