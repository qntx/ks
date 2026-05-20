//! Interactive terminal prompts.
//!
//! Centralises all `cliclack` usage so command modules stay focused on their
//! domain logic.

use std::io::IsTerminal as _;

use cliclack::{confirm as ck_confirm, password as ck_password};
use ks::{Error, Result};
use secrecy::SecretString;
use zeroize::Zeroizing;

const MIN_PASSPHRASE: usize = 12;

/// Reads a passphrase masked with `•`.
///
/// # Errors
/// Returns [`Error::Io`] if the terminal cannot be read (e.g. closed stdin).
pub fn passphrase(label: &str) -> Result<SecretString> {
    let raw: String = ck_password(label).mask('•').interact().map_err(io_err)?;
    Ok(SecretString::from(raw))
}

/// Reads and confirms a new passphrase. Requires the two entries to match and
/// asks the user to confirm if the chosen passphrase is shorter than
/// [`MIN_PASSPHRASE`] characters.
///
/// # Errors
/// Returns [`Error::Io`] on prompt failure or [`Error::WrongPassphrase`] if
/// the two entries disagree.
pub fn new_passphrase(label: &str) -> Result<SecretString> {
    let a: String = ck_password(label).mask('•').interact().map_err(io_err)?;
    let b: String = ck_password("Confirm passphrase")
        .mask('•')
        .interact()
        .map_err(io_err)?;
    if a != b {
        return Err(Error::WrongPassphrase);
    }
    if a.chars().count() < MIN_PASSPHRASE {
        let proceed = ck_confirm(format!(
            "Passphrase is shorter than {MIN_PASSPHRASE} characters — continue anyway?"
        ))
        .initial_value(false)
        .interact()
        .map_err(io_err)?;
        if !proceed {
            return Err(Error::Io(std::io::Error::other("aborted by user")));
        }
    }
    Ok(SecretString::from(a))
}

/// Asks a yes/no question.
///
/// # Errors
/// Returns [`Error::Io`] on prompt failure.
pub fn confirm(label: &str, default: bool) -> Result<bool> {
    ck_confirm(label)
        .initial_value(default)
        .interact()
        .map_err(io_err)
}

/// Reads a secret value, either from an interactive masked prompt or, if
/// stdin is not a TTY, from the first line of stdin.
///
/// # Errors
/// Returns [`Error::Io`] on read failure.
pub fn secret_value(label: &str) -> Result<Zeroizing<String>> {
    if std::io::stdin().is_terminal() {
        let raw: String = ck_password(label).mask('•').interact().map_err(io_err)?;
        Ok(Zeroizing::new(raw))
    } else {
        let mut buf = String::new();
        std::io::stdin().read_line(&mut buf).map_err(Error::Io)?;
        let trimmed = buf.trim_end_matches(['\n', '\r']).to_owned();
        Ok(Zeroizing::new(trimmed))
    }
}

const fn io_err(e: std::io::Error) -> Error {
    Error::Io(e)
}
