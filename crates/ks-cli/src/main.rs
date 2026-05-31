//! `ks` -- Key Store CLI entry point.

#![allow(
    unreachable_pub,
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::exit,
    reason = "binary crate: `pub` items are internal; a CLI legitimately writes to stdout/stderr and exits with structured non-zero codes"
)]

mod audit;
mod cli;
mod clipboard;
mod commands;
mod exit;
mod hardening;
mod output;
mod prompt;
mod terminal;

use std::process::ExitCode;

use clap::Parser;

fn main() -> ExitCode {
    hardening::harden();
    let cli = cli::Cli::parse();
    output::init(cli.json);
    match commands::dispatch(cli) {
        Ok(code) => code,
        Err(e) => {
            output::error(&e);
            ExitCode::from(exit::for_error(&e).as_u8())
        }
    }
}
