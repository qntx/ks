//! Stable, scriptable exit codes.
//!
//! Numbers follow [BSD `sysexits.h`](https://man.freebsd.org/cgi/man.cgi?sysexits)
//! conventions, with one extension: `1` is reserved for the unspecified
//! "general error" used by most Unix tools so users can keep their existing
//! `&&` chains intact.

use ks::Error;

/// Exit codes produced when `ks` fails.
///
/// On success we return [`std::process::ExitCode::SUCCESS`] (0) directly from
/// each command; this enum is the structured failure surface, mapped from
/// [`Error`] by [`for_error`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ExitCode {
    /// Generic, unspecified failure.
    Failure = 1,
    /// User supplied wrong arguments / paths / etc.
    Usage = 64,
    /// Data format error (corrupt file, malformed TOTP, bad recipient line).
    DataErr = 65,
    /// An input file did not exist (store/identity/secret not found).
    NoInput = 66,
    /// An internal logic bug (crypto failure that should not happen).
    Software = 70,
    /// Cannot create a file or directory (already exists, permission, ...).
    CantCreat = 73,
    /// Permission denied (wrong passphrase / no perms on file).
    NoPerm = 77,
}

impl ExitCode {
    /// Returns the numeric value as `u8` for [`std::process::ExitCode::from`].
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

/// Maps a library [`Error`] to the closest matching [`ExitCode`].
#[must_use]
pub const fn for_error(err: &Error) -> ExitCode {
    match err {
        Error::WrongPassphrase => ExitCode::NoPerm,

        Error::StoreNotFound(_)
        | Error::IdentityNotFound(_)
        | Error::NoRecipients(_)
        | Error::SecretNotFound(_) => ExitCode::NoInput,

        Error::StoreExists(_) | Error::IdentityExists(_) | Error::SecretExists(_) => {
            ExitCode::CantCreat
        }

        Error::InvalidPath(_) | Error::InvalidRecipient(_) | Error::InvalidArgument(_) => {
            ExitCode::Usage
        }

        Error::InvalidTotp(_) | Error::Tampered { .. } => ExitCode::DataErr,

        Error::Encrypt(_) | Error::Decrypt(_) | Error::NoUserDir => ExitCode::Software,

        // `ks::Error` is `#[non_exhaustive]`; Io, Command and any future
        // variants all map to the generic `Failure` exit code.
        _ => ExitCode::Failure,
    }
}
