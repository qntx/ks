//! `ks identity` --inspect or export the local identity.

use std::path::Path;
use std::process::ExitCode;

use ks::{Config, Error, Result};

use crate::cli::IdentityCmd;
use crate::commands;
use crate::terminal;

pub fn run(config: &Config, cmd: IdentityCmd) -> Result<ExitCode> {
    match cmd {
        IdentityCmd::Show => {
            // Showing the public recipient does not require unlocking the
            // identity file: we can derive it once we have the identity.
            let identity = commands::unlock(config)?;
            println!("{}", identity.to_public());
            Ok(ExitCode::SUCCESS)
        }
        IdentityCmd::Export { dest } => export(&config.identity_path, &dest),
    }
}

fn export(src: &Path, dst: &Path) -> Result<ExitCode> {
    if !src.exists() {
        return Err(Error::IdentityNotFound(src.to_path_buf()));
    }
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(src, dst)?;
    terminal::success(&format!(
        "Exported encrypted identity to {} (still passphrase-protected)",
        dst.display()
    ));
    Ok(ExitCode::SUCCESS)
}
