//! `ks init` — bootstrap a new identity and store.

use std::process::ExitCode;

use cliclack::{intro, outro};
use ks::{Config, Result, Store, agent, git as git_, identity};

use crate::prompt;
use crate::terminal;

pub fn run(config: Config, init_git: bool) -> Result<ExitCode> {
    intro("ks — initialise key store")?;

    let pp = prompt::new_passphrase("Choose a master passphrase")?;
    let id = identity::create(&config.identity_path, pp)?;
    let store = Store::create(config.clone(), id, &[])?;

    let _ = agent::put(
        store.root(),
        store.identity(),
        config.tunables.session_ttl_secs,
    );

    terminal::success(&format!(
        "Identity written to {}",
        config.identity_path.display()
    ));
    terminal::success(&format!("Store created at {}", store.root().display()));
    terminal::info(&format!(
        "Public key (recipient): {}",
        store.identity().to_public()
    ));

    if init_git {
        git_::init(store.root())?;
        terminal::success("Initialised git repository in store");
    }

    outro("Use `ks set <path>` to store your first secret.")?;
    Ok(ExitCode::SUCCESS)
}
