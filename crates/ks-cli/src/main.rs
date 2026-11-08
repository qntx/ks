//! `ks` — personal local secret vault with an age-compatible on-disk format.

#![allow(
    unsafe_code,
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::must_use_candidate,
    clippy::wildcard_imports,
    clippy::needless_pass_by_value,
    clippy::needless_pass_by_ref_mut,
    clippy::needless_return,
    clippy::case_sensitive_file_extension_comparisons,
    clippy::unnecessary_wraps,
    clippy::items_after_statements,
    clippy::shadow_unrelated,
    clippy::indexing_slicing,
    clippy::collapsible_if,
    clippy::match_same_arms,
    clippy::option_if_let_else,
    clippy::or_fun_call,
    clippy::let_underscore_must_use,
    clippy::unnecessary_to_owned,
    clippy::unnecessary_debug_formatting,
    clippy::multiple_unsafe_ops_per_block,
    clippy::missing_const_for_fn,
    clippy::elidable_lifetime_names,
    clippy::borrow_as_ptr,
    clippy::too_long_first_doc_paragraph,
    clippy::let_and_return,
    clippy::disallowed_methods,
    reason = "CLI glue code: printing, terse control flow, small owned values, and blocking waits in the clipboard daemon are intentional; the only unsafe is scoped in harden.rs"
)]

pub mod cli;
pub mod clipboard;
pub mod commands;
pub mod harden;
pub mod output;
pub mod prompt;

#[cfg(test)]
use assert_cmd as _;
use clap::Parser as _;
use output::CliResult;

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
                        &e.to_string().trim_end().to_owned(),
                    );
                }
            }
        }
    };
    let mut out = output::Out::new(cli.json);
    let result: CliResult<i32> = dispatch(&cli, &mut out);
    match result {
        Ok(code) => code,
        Err((code, message)) => out.error(code, &message),
    }
}

fn dispatch(cli: &cli::Cli, out: &mut output::Out) -> CliResult<i32> {
    use cli::Command as C;
    match &cli.command {
        C::Init => commands::init::run(cli, out),
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
        C::ClipboardDaemon { secs } => commands::daemon::run_daemon(*secs),
    }
}
