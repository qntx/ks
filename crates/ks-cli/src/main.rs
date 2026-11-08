//! `ks` — personal local secret vault with an age-compatible on-disk format.

mod cli;
mod clipboard;
mod commands;
mod harden;
mod output;
mod prompt;

#[cfg(test)]
use assert_cmd as _;
use clap::Parser as _;
use output::CliResult;

#[allow(
    clippy::disallowed_methods,
    reason = "the binary must exit with the mapped code; there is no Termination path that preserves it"
)]
fn main() {
    harden::run();
    let code = run();
    std::process::exit(code);
}

fn run() -> i32 {
    let argv_has_json = std::env::args().any(|a| a == "--json");
    let cli = match cli::Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            use clap::error::ErrorKind as K;
            match e.kind() {
                K::DisplayHelp | K::DisplayVersion => {
                    e.print().ok();
                    return 0;
                }
                _ => {
                    return output::exit_error(
                        argv_has_json,
                        ks::ErrorCode::Usage,
                        String::from(e.to_string().trim_end()).as_str(),
                    );
                }
            }
        }
    };
    let out = output::Out::new(cli.json);
    let result: CliResult<i32> = dispatch(&cli, &out);
    match result {
        Ok(code) => code,
        Err((code, message)) => out.error(code, &message),
    }
}

fn dispatch(cli: &cli::Cli, out: &output::Out) -> CliResult<i32> {
    use cli::Command as C;
    match &cli.command {
        C::Init { git, clone } => commands::init::run(cli, out, *git, clone.as_deref()),
        C::Ls { prefix } => commands::ls::run(cli, out, prefix.as_deref()),
        C::Show { name, field, copy } => {
            commands::show::run(cli, out, name, field.as_deref(), *copy)
        }
        C::Set {
            name,
            multiline,
            binary,
            force,
        } => commands::set::run(cli, out, name, *multiline, *binary, *force),
        C::Edit { name } => commands::edit::run(cli, out, name),
        C::Gen {
            name,
            length,
            set,
            copy,
            force,
        } => commands::generate::run(
            cli,
            out,
            name.as_deref(),
            *length,
            set.as_str(),
            *copy,
            *force,
        ),
        C::Rm { name, force } => commands::rm::run(cli, out, name, *force),
        C::Mv { from, to, force } => commands::mv::run(cli, out, from, to, *force),
        C::Otp { name, copy } => commands::otp::run(cli, out, name, *copy),
        C::Run { env, command } => commands::run::run(cli, out, env, command),
        C::Passwd => commands::passwd::run(cli, out),
        C::Doctor { fix } => commands::doctor::run(cli, out, *fix),
        C::Sync { keep } => commands::sync::run(cli, out, keep.as_deref()),
        C::Git { args } => commands::git::run(cli, out, args),
        C::ClipboardDaemon { secs } => Ok(commands::daemon::run_daemon(*secs)),
    }
}
