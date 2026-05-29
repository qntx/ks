//! `ks init` — bootstrap a new identity and store.

use std::io::IsTerminal as _;
use std::process::ExitCode;

use cliclack::{intro, outro};
use ks::{Config, Result, Store, crypto, git as git_};
use secrecy::SecretString;

use crate::prompt;
use crate::terminal;

pub fn run(config: &Config, init_git: bool) -> Result<ExitCode> {
    let interactive = std::io::stdin().is_terminal();

    let pp = match std::env::var("KS_PASSPHRASE") {
        Ok(raw) if !raw.is_empty() => SecretString::from(raw),
        _ => {
            intro("ks: initialise key store")?;
            prompt::new_passphrase("Choose a master passphrase")?
        }
    };
    let id = crypto::create_identity(&config.identity_path, pp)?;
    let store = Store::create(config.clone(), &id, &[])?;

    terminal::success(&format!(
        "Identity written to {}",
        config.identity_path.display()
    ));
    terminal::success(&format!("Store created at {}", store.root().display()));
    terminal::info(&format!("Public key (recipient): {}", id.to_public()));

    if init_git {
        git_::init(store.root())?;
        terminal::success("Initialised git repository in store");
    }

    if interactive {
        outro("Use `ks insert <path>` to store your first secret.")?;
    }
    Ok(ExitCode::SUCCESS)
}
