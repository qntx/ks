//! `ks passwd` — change the identity passphrase.

use std::process::ExitCode;

use cliclack::intro;
use ks::{Config, Result, agent, identity};

use crate::prompt;
use crate::terminal;

pub fn run(config: Config) -> Result<ExitCode> {
    intro("ks — change passphrase")?;

    let current = prompt::passphrase("Current passphrase")?;
    let new = prompt::new_passphrase("New passphrase")?;
    identity::change_passphrase(&config.identity_path, current, new)?;

    // Force the next operation to re-derive the cached identity.
    agent::clear(&config.store_dir)?;

    terminal::success("Passphrase updated; session cleared");
    Ok(ExitCode::SUCCESS)
}
