//! Subcommand implementations and the top-level dispatcher.

use std::process::ExitCode;

use ks::x25519;
use ks::{Config, Result, Store, crypto};
use secrecy::SecretString;

use crate::cli::{Cli, Command};
use crate::prompt;

pub mod cp;
pub mod doctor;
pub mod edit;
pub mod generate;
pub mod git;
pub mod grep;
pub mod identity;
pub mod init;
pub mod insert;
pub mod ls;
pub mod mv;
pub mod otp;
pub mod passwd;
pub mod recipients;
pub mod rm;
pub mod run;
pub mod show;

/// Dispatches a parsed [`Cli`] to the matching command implementation.
///
/// # Errors
/// Propagates any [`ks::Error`] from the underlying command.
pub fn dispatch(cli: Cli) -> Result<ExitCode> {
    let config = Config::load()?;
    let cfg = &config;
    match cli.command {
        Command::Init { git } => init::run(cfg, git),
        Command::Ls { prefix } => ls::run(cfg, &prefix),
        Command::Show {
            path,
            copy,
            field,
            meta,
        } => show::run(cfg, &path, copy, field.as_deref(), meta),
        Command::Insert {
            path,
            multiline,
            force,
        } => insert::run(cfg, &path, multiline, force),
        Command::Edit { path } => edit::run(cfg, &path),
        Command::Gen {
            path,
            length,
            charset,
            force,
            copy,
        } => generate::run(cfg, path.as_deref(), length, charset, force, copy),
        Command::Rm { path, force } => rm::run(cfg, &path, force),
        Command::Mv { from, to } => mv::run(cfg, &from, &to),
        Command::Cp { from, to } => cp::run(cfg, &from, &to),
        Command::Grep { query, values } => grep::run(cfg, &query, values),
        Command::Otp { path, copy } => otp::run(cfg, &path, copy),
        Command::Run { env, prefix, cmd } => run::run(cfg, &env, &prefix, &cmd),
        Command::Recipients { cmd } => recipients::run(cfg, cmd),
        Command::Identity => identity::run(cfg),
        Command::Git { args } => git::run(cfg, &args),
        Command::Doctor => Ok(doctor::run(cfg)),
        Command::Passwd => passwd::run(cfg),
    }
}

/// Unlocks the identity from `KS_PASSPHRASE` or an interactive masked prompt.
///
/// Nothing is cached: writing secrets never needs this, and reads prompt once
/// per invocation (set `KS_PASSPHRASE` for non-interactive use).
///
/// # Errors
/// - [`ks::Error::IdentityNotFound`] if no identity file exists.
/// - [`ks::Error::WrongPassphrase`] for a bad passphrase.
pub fn unlock(config: &Config) -> Result<x25519::Identity> {
    if let Ok(raw) = std::env::var("KS_PASSPHRASE")
        && !raw.is_empty()
    {
        return crypto::load_identity(&config.identity_path, SecretString::from(raw));
    }
    let pp = prompt::passphrase("Enter passphrase")?;
    crypto::load_identity(&config.identity_path, pp)
}

/// Opens the store (recipients only — no passphrase prompt).
///
/// # Errors
/// See [`Store::open`].
pub fn open_store(config: &Config) -> Result<Store> {
    Store::open(config.clone())
}

/// Clipboard auto-clear delay in seconds, from `KS_CLIP_TIME` (default 45).
pub(crate) fn clip_secs() -> u64 {
    std::env::var("KS_CLIP_TIME")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(45)
}
