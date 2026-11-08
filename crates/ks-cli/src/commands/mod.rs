//! Command implementations.

pub(crate) mod daemon;
pub(crate) mod doctor;
pub(crate) mod edit;
#[path = "gen.rs"]
pub(crate) mod generate;
pub(crate) mod init;
pub(crate) mod ls;
pub(crate) mod mv;
pub(crate) mod otp;
pub(crate) mod passwd;
pub(crate) mod rm;
pub(crate) mod run;
pub(crate) mod set;
pub(crate) mod show;

pub(crate) use crate::cli::Cli;
pub(crate) use crate::clipboard;
pub(crate) use crate::output::{CliResult, Out, err};
pub(crate) use crate::prompt;
pub(crate) use ks::{ErrorCode, Name, Vault};

/// Opens the vault directory (no unlock).
pub(crate) fn vault(cli: &Cli) -> CliResult<Vault> {
    let path = cli.vault_path().map_err(|e| err(&e))?;
    Vault::open(&path).map_err(|e| err(&e))
}

/// Unlocks the vault with the passphrase source chain.
pub(crate) fn unlock(cli: &Cli, out: &Out) -> CliResult<ks::Session> {
    let v = vault(cli)?;
    let source = prompt::passphrase_source(cli, out);
    let session = v.unlock(Some(&source)).map_err(|e| err(&e))?;
    for w in session.warnings() {
        out.warn(w.clone());
    }
    Ok(session)
}

/// Parses an entry name, mapping failures to `usage`.
pub(crate) fn name(s: &str) -> CliResult<Name> {
    Name::parse(s).map_err(|e| err(&e))
}

/// Maps a raw (code, message) for non-library errors.
pub(crate) fn fail<T>(code: ErrorCode, msg: impl Into<String>) -> CliResult<T> {
    Err((code, msg.into()))
}
