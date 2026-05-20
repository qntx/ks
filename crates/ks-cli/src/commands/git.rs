//! `ks git` — thin wrapper for syncing the store with git.

use std::process::ExitCode;

use ks::{Config, Error, Result, git};

use crate::cli::GitCmd;
use crate::terminal;

pub fn run(config: Config, cmd: GitCmd) -> Result<ExitCode> {
    let dir = &config.store_dir;
    if !dir.exists() {
        return Err(Error::StoreNotFound(dir.clone()));
    }

    match cmd {
        GitCmd::Init => {
            git::init(dir)?;
            terminal::success(&format!("Initialised git repository in {}", dir.display()));
        }
        GitCmd::Sync { message } => {
            require_repo(dir)?;
            git::add_all(dir)?;
            git::commit(dir, &message)?;
            // Best-effort pull/push: if no remote is configured these will fail
            // and we surface the underlying git error.
            git::pull_rebase(dir)?;
            git::push(dir)?;
            terminal::success("Sync complete");
        }
        GitCmd::Status => {
            require_repo(dir)?;
            let out = git::status(dir)?;
            print!("{out}");
        }
        GitCmd::Log { n } => {
            require_repo(dir)?;
            let out = git::log(dir, n)?;
            print!("{out}");
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn require_repo(dir: &std::path::Path) -> Result<()> {
    if !git::is_repo(dir) {
        return Err(Error::Command {
            cmd: "git".into(),
            status: 128,
            stderr: format!(
                "store at {} is not a git repository; run `ks git init` first",
                dir.display()
            ),
        });
    }
    Ok(())
}
