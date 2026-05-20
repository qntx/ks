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
/// Returns an [`ExitCode`] --usually `Ok(0)` --propagated to `main`.
///
/// # Errors
/// Propagates any [`ks::Error`] from the underlying command.
pub fn dispatch(cli: Cli) -> Result<ExitCode> {
    let config = Config::load()?;
    let cfg = &config;
    match cli.command {
        Command::Init { git } => init::run(cfg, git),
        Command::Get { path, copy, field } => get::run(cfg, &path, copy, field.as_deref()),
        Command::Set {
            path,
            note,
            force,
            totp,
        } => set::run(cfg, &path, note.as_deref(), force, totp),
        Command::Rm { path, force } => rm::run(cfg, &path, force),
        Command::Ls { prefix } => ls::run(cfg, &prefix),
        Command::Mv { from, to } => mv::run(cfg, &from, &to),
        Command::Find { query, notes } => find::run(cfg, &query, notes),
        Command::Info { path } => info::run(cfg, &path),
        Command::Gen {
            path,
            length,
            charset,
            force,
            copy,
        } => pwgen::run(cfg, path.as_deref(), length, &charset, force, copy),
        Command::Otp { path, copy } => otp::run(cfg, &path, copy),
        Command::Run { env, prefix, cmd } => run::run(cfg, &env, &prefix, &cmd),
        Command::Inject { input, output } => inject::run(cfg, input.as_deref(), output.as_deref()),
        Command::Env { targets, shell } => env_::run(cfg, &targets, &shell),
        Command::Recipients { cmd } => recipients::run(cfg, cmd),
        Command::Identity { cmd } => identity_::run(cfg, cmd),
        Command::Git { cmd } => git::run(cfg, cmd),
        Command::Doctor => Ok(doctor::run(cfg)),
        Command::Passwd => passwd::run(cfg),
        Command::Lock => lock::run(cfg),
        Command::Unlock => unlock::run(cfg),
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
        cache_session(config, &id);
        return Ok(id);
    }

    if let Some(cached) = agent::get(&config.store_dir) {
        return Ok(cached);
    }

    let pp = prompt::passphrase("Enter passphrase")?;
    let id = identity::load(&config.identity_path, pp)?;
    cache_session(config, &id);
    Ok(id)
}

/// Best-effort session cache: failures (e.g. keyring unavailable) emit a soft
/// warning rather than fail the command.
pub(crate) fn cache_session(config: &Config, identity: &x25519::Identity) {
    if let Err(e) = agent::put(
        &config.store_dir,
        identity,
        config.tunables.session_ttl_secs,
    ) {
        crate::terminal::warn(&format!("session cache unavailable: {e}"));
    }
}

/// Unlocks the identity and opens the [`Store`].
///
/// # Errors
/// See [`unlock`] and [`Store::open`].
pub fn open_store(config: &Config) -> Result<Store> {
    let identity = unlock(config)?;
    Store::open(config.clone(), identity)
}
