//! Command implementations.

pub mod daemon;
pub mod doctor;
pub mod edit;
#[path = "gen.rs"]
pub mod generate;
pub mod init;
pub mod ls;
pub mod mv;
pub mod otp;
pub mod passwd;
pub mod rm;
pub mod run;
pub mod set;
pub mod show;

pub use crate::cli::Cli;
pub use crate::clipboard;
pub use crate::output::{CliResult, Out, err};
pub use crate::prompt;
pub use ks::{ErrorCode, Name, Vault};

/// Opens the vault directory (no unlock).
pub fn vault(cli: &Cli) -> CliResult<Vault> {
    let path = cli.vault_path().map_err(|e| err(&e))?;
    Vault::open(&path).map_err(|e| err(&e))
}

/// Unlocks the vault with the passphrase source chain.
pub fn unlock(cli: &Cli, out: &mut Out) -> CliResult<ks::Session> {
    let v = vault(cli)?;
    let source = prompt::passphrase_source(cli, out);
    let session = v.unlock(Some(&source)).map_err(|e| err(&e))?;
    for w in session.warnings() {
        out.warn(w.clone());
    }
    Ok(session)
}

/// Parses an entry name, mapping failures to `usage`.
pub fn name(s: &str) -> CliResult<Name> {
    Name::parse(s).map_err(|e| err(&e))
}

/// Maps a raw (code, message) for non-library errors.
pub fn fail<T>(code: ErrorCode, msg: impl Into<String>) -> CliResult<T> {
    Err((code, msg.into()))
}
