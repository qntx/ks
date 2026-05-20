//! `ks` -- Key Store CLI entry point.

#![allow(
    unreachable_pub,
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::exit,
    reason = "binary crate: `pub` items are internal; a CLI legitimately writes to stdout/stderr and exits with structured non-zero codes"
)]

mod cli;
mod clipboard;
mod commands;
mod exit;
mod prompt;
mod terminal;

use std::process::ExitCode;

use clap::Parser;

fn main() -> ExitCode {
    // Register the platform-native credential store with keyring-core so
    // the session cache (`ks::agent`) can transparently store/retrieve
    // unlocked identities. Best-effort: if the OS keyring is unavailable
    // we silently degrade to "no cache" mode and prompt every time.
    //
    // `not_keyutils = false` keeps the default of `keyutils` on Linux
    // (kernel session keyring, scoped to the login session) which matches
    // the semantics of a TTL-bound session cache. On other platforms the
    // argument is ignored.
    if let Err(_e) = keyring::use_native_store(false) {}

    let cli = cli::Cli::parse();
    match commands::dispatch(cli) {
        Ok(code) => code,
        Err(e) => {
            terminal::error(&e.to_string());
            ExitCode::from(exit::for_error(&e).as_u8())
        }
    }
}
