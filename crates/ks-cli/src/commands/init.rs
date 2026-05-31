//! `ks init` — bootstrap a new identity and store.

use std::io::IsTerminal as _;
use std::process::ExitCode;

use cliclack::{intro, outro};
use ks::{Config, Result, Store, crypto, git as git_};
use secrecy::SecretString;

use crate::prompt;
use crate::terminal;

pub fn run(config: &Config, init_git: bool) -> Result<ExitCode> {
    let json = crate::output::is_json();
    let interactive = std::io::stdin().is_terminal() && !json;

    let pp = if let Some(raw) = crate::hardening::take_env("KS_PASSPHRASE") {
        SecretString::from(raw)
    } else if json {
        return Err(ks::Error::InvalidArgument(
            "KS_PASSPHRASE is required to set the master passphrase in --json mode".to_owned(),
        ));
    } else {
        intro("ks: initialise key store")?;
        prompt::new_passphrase("Choose a master passphrase")?
    };
    let id = crypto::create_identity(&config.identity_path, pp)?;
    let store = Store::create(config.clone(), &id, &[])?;
    if init_git {
        git_::init(store.root())?;
    }

    if json {
        crate::output::emit(&serde_json::json!({
            "identity_path": config.identity_path.display().to_string(),
            "store_dir": store.root().display().to_string(),
            "public_key": id.to_public().to_string(),
            "git": init_git,
        }));
        return Ok(ExitCode::SUCCESS);
    }

    terminal::success(&format!(
        "Identity written to {}",
        config.identity_path.display()
    ));
    terminal::success(&format!("Store created at {}", store.root().display()));
    terminal::info(&format!("Public key (recipient): {}", id.to_public()));
    if init_git {
        terminal::success("Initialised git repository in store");
    }
    if interactive {
        outro("Use `ks insert <path>` to store your first secret.")?;
    }
    Ok(ExitCode::SUCCESS)
}
