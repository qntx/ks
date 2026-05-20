//! `ks unlock` --force-unlock and cache the session identity.

use std::process::ExitCode;

use ks::{Config, Result, agent};

use crate::commands;
use crate::terminal;

pub fn run(config: &Config) -> Result<ExitCode> {
    // Drop any stale cache so the explicit unlock always prompts (or uses
    // KS_PASSPHRASE) and starts a fresh TTL window.
    agent::clear(&config.store_dir)?;
    let _identity = commands::unlock(config)?;
    terminal::success(&format!(
        "Session unlocked for {}s",
        config.tunables.session_ttl_secs
    ));
    Ok(ExitCode::SUCCESS)
}
