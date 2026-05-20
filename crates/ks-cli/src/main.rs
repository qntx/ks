//! Key Store CLI — `ks` binary entry point.
#![allow(
    unreachable_pub,
    missing_docs,
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::exit
)]

mod clip;
mod cmd;
mod output;

use clap::Parser as _;
use owo_colors::OwoColorize as _;

use cmd::{Cli, Command};

fn main() {
    let cli = Cli::parse();

    let config = match ks::Config::load() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{} {e}", "✗".red().bold());
            std::process::exit(1);
        }
    };

    let result = match cli.command {
        Command::Init => cmd::init::run(&config),

        Command::Get { path, copy } => cmd::get::run(&config, &path, copy),

        Command::Set { path, note, force } => {
            cmd::set::run(&config, &path, note.as_deref(), force)
        }

        Command::Del { path, force } => cmd::del::run(&config, &path, force),

        Command::Ls { prefix } => cmd::list::run(&config, &prefix),

        Command::Gen {
            path,
            length,
            charset,
            force,
        } => cmd::generate::run(&config, path.as_deref(), length, &charset, force),

        Command::Env { paths, shell } => cmd::env::run(&config, &paths, &shell),

        Command::Find { query } => cmd::find::run(&config, &query),

        Command::Info { path } => cmd::info::run(&config, &path),

        Command::Passwd => cmd::passwd::run(&config),

        Command::Export { output } => cmd::io::run_export(&config, output.as_deref()),

        Command::Import { file, dotenv } => {
            cmd::io::run_import(&config, file.as_deref(), dotenv)
        }

        Command::Lock => cmd::lock::run(),
    };

    if let Err(e) = result {
        output::print_error(&e.to_string());
        std::process::exit(1);
    }
}
