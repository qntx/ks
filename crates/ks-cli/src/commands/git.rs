//! `ks git` — passthrough to the system `git`, run inside the store directory.

use std::process::{Command as Proc, ExitCode};

use ks::{Config, Error, Result};

pub fn run(config: &Config, args: &[String]) -> Result<ExitCode> {
    let status = Proc::new("git")
        .arg("-C")
        .arg(&config.store_dir)
        .args(args)
        .status()
        .map_err(Error::Io)?;
    Ok(crate::commands::child_exit_code(status))
}
