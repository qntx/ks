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
    let cli = cli::Cli::parse();
    match commands::dispatch(cli) {
        Ok(code) => code,
        Err(e) => {
            terminal::error(&e.to_string());
            ExitCode::from(exit::for_error(&e).as_u8())
        }
    }
}
