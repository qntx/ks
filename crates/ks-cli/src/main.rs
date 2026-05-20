//! `ks` — Key Store CLI entry point.

#![allow(
    unreachable_pub,
    missing_docs,
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::exit
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
