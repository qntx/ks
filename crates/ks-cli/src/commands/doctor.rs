//! `ks doctor` — sanity-check the store, identity, recipients and git state.

use std::io::IsTerminal as _;
use std::path::Path;
use std::process::ExitCode;

use ks::{Config, crypto, git};
use owo_colors::{OwoColorize as _, Stream, Style};

use crate::commands;

pub fn run(config: &Config) -> ExitCode {
    let mut failures: usize = 0;

    check(
        "identity file present",
        config.identity_path.exists(),
        &config.identity_path.display().to_string(),
        &mut failures,
    );
    check(
        "store directory present",
        config.store_dir.exists(),
        &config.store_dir.display().to_string(),
        &mut failures,
    );

    let recipients_path = config.recipients_path();
    let recipients_ok =
        recipients_path.exists() && crypto::load_recipients(&recipients_path).is_ok();
    check(
        ".age-recipients valid",
        recipients_ok,
        &recipients_path.display().to_string(),
        &mut failures,
    );

    check_identity(config, &recipients_path, &mut failures);

    if git::is_repo(&config.store_dir) {
        match git::status(&config.store_dir) {
            Ok(out) => {
                eprintln!(
                    "  {} git status:",
                    "[*]".if_supports_color(Stream::Stderr, |t| t.cyan()),
                );
                for line in out.lines() {
                    eprintln!("    {line}");
                }
            }
            Err(e) => check("git status", false, &e.to_string(), &mut failures),
        }
    } else {
        eprintln!(
            "  {} git: not a repo",
            "[ ]".if_supports_color(Stream::Stderr, |t| t.dimmed()),
        );
    }

    if failures == 0 {
        eprintln!(
            "\n{} all checks passed",
            "[OK]".if_supports_color(Stream::Stderr, |t| t.style(Style::new().green().bold())),
        );
        ExitCode::SUCCESS
    } else {
        eprintln!(
            "\n{} {} check(s) failed",
            "[FAIL]".if_supports_color(Stream::Stderr, |t| t.style(Style::new().red().bold())),
            failures
                .to_string()
                .if_supports_color(Stream::Stderr, |t| t.bold()),
        );
        ExitCode::from(1)
    }
}

fn check(label: &str, ok: bool, detail: &str, failures: &mut usize) {
    let mark = if ok {
        "[OK]"
            .if_supports_color(Stream::Stderr, |t| t.style(Style::new().green().bold()))
            .to_string()
    } else {
        "[FAIL]"
            .if_supports_color(Stream::Stderr, |t| t.style(Style::new().red().bold()))
            .to_string()
    };
    eprintln!("  {mark} {label}: {detail}");
    if !ok {
        *failures = failures.saturating_add(1);
    }
}

/// Verifies the identity unlocks and is present in the recipient list.
///
/// Skipped when neither `KS_PASSPHRASE` nor an interactive terminal is
/// available, so `ks doctor` stays non-blocking in scripts and CI.
fn check_identity(config: &Config, recipients_path: &Path, failures: &mut usize) {
    let can_unlock = std::env::var("KS_PASSPHRASE").is_ok_and(|v| !v.is_empty())
        || std::io::stdin().is_terminal();
    if !can_unlock {
        eprintln!(
            "  {} identity unlocks: skipped (set KS_PASSPHRASE for non-interactive checks)",
            "[--]".if_supports_color(Stream::Stderr, |t| t.dimmed()),
        );
        return;
    }
    let identity = match commands::unlock(config) {
        Ok(id) => id,
        Err(e) => {
            check("identity unlocks", false, &e.to_string(), failures);
            return;
        }
    };
    check("identity unlocks", true, "ok (env or prompt)", failures);
    if let Ok(list) = crypto::load_recipients(recipients_path) {
        let own = identity.to_public();
        check(
            "identity is in .age-recipients",
            crypto::recipients_contain(&list, &own),
            &own.to_string(),
            failures,
        );
    }
}
