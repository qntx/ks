//! Subcommand implementations and the top-level dispatcher.

use std::process::ExitCode;

use ks::x25519;
use ks::{Config, Result, Store, agent, identity};
use secrecy::SecretString;

use crate::cli::{Cli, Command};
use crate::prompt;

pub mod doctor;
pub mod env_;
pub mod find;
pub mod get;
pub mod git;
pub mod identity_;
pub mod info;
pub mod init;
pub mod inject;
pub mod lock;
pub mod ls;
pub mod mv;
pub mod otp;
pub mod passwd;
pub mod pwgen;
pub mod recipients;
pub mod rm;
pub mod run;
pub mod set;
pub mod unlock;

/// Dispatches a parsed [`Cli`] to the matching command implementation.
///
/// Returns an [`ExitCode`] — usually `Ok(0)` — propagated to `main`.
///
/// # Errors
/// Propagates any [`ks::Error`] from the underlying command.
pub fn dispatch(cli: Cli) -> Result<ExitCode> {
    let config = Config::load()?;
    match cli.command {
        Command::Init { git } => init::run(config, git),
        Command::Get { path, copy, field } => get::run(config, &path, copy, field.as_deref()),
        Command::Set {
            path,
            note,
            force,
            totp,
        } => set::run(config, &path, note.as_deref(), force, totp),
        Command::Rm { path, force } => rm::run(config, &path, force),
        Command::Ls { prefix } => ls::run(config, &prefix),
        Command::Mv { from, to } => mv::run(config, &from, &to),
        Command::Find { query, notes } => find::run(config, &query, notes),
        Command::Info { path } => info::run(config, &path),
        Command::Gen {
            path,
            length,
            charset,
            force,
            copy,
        } => pwgen::run(config, path.as_deref(), length, &charset, force, copy),
        Command::Otp { path, copy } => otp::run(config, &path, copy),
        Command::Run { env, prefix, cmd } => run::run(config, &env, &prefix, &cmd),
        Command::Inject { input, output } => {
            inject::run(config, input.as_deref(), output.as_deref())
        }
        Command::Env { targets, shell } => env_::run(config, &targets, &shell),
        Command::Recipients { cmd } => recipients::run(config, cmd),
        Command::Identity { cmd } => identity_::run(config, cmd),
        Command::Git { cmd } => git::run(config, cmd),
        Command::Doctor => doctor::run(config),
        Command::Passwd => passwd::run(config),
        Command::Lock => lock::run(config),
        Command::Unlock => unlock::run(config),
    }
}

/// Unlocks the identity for `config` from (in order):
/// 1. `KS_PASSPHRASE` environment variable;
/// 2. an unexpired OS keyring session entry;
/// 3. an interactive masked prompt.
///
/// On a successful prompt, the resulting identity is cached in the keyring
/// for `config.tunables.session_ttl_secs` seconds.
///
/// # Errors
/// - [`ks::Error::IdentityNotFound`] if no identity file exists.
/// - [`ks::Error::WrongPassphrase`] for a bad passphrase.
pub fn unlock(config: &Config) -> Result<x25519::Identity> {
    if let Ok(raw) = std::env::var("KS_PASSPHRASE")
        && !raw.is_empty()
    {
        let id = identity::load(&config.identity_path, SecretString::from(raw))?;
        let _ = agent::put(&config.store_dir, &id, config.tunables.session_ttl_secs);
        return Ok(id);
    }

    if let Some(cached) = agent::get(&config.store_dir) {
        return Ok(cached);
    }

    let pp = prompt::passphrase("Enter passphrase")?;
    let id = identity::load(&config.identity_path, pp)?;
    let _ = agent::put(&config.store_dir, &id, config.tunables.session_ttl_secs);
    Ok(id)
}

/// Unlocks the identity and opens the [`Store`].
///
/// # Errors
/// See [`unlock`] and [`Store::open`].
pub fn open_store(config: Config) -> Result<Store> {
    let identity = unlock(&config)?;
    Store::open(config, identity)
}
