//! `ks doctor` — sanity-check the store, identity, recipients and git state.

use std::process::ExitCode;

use ks::recipient;
use ks::{Config, Result, agent, git};
use owo_colors::OwoColorize as _;

use crate::commands;

pub fn run(config: Config) -> Result<ExitCode> {
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
    let recipients_ok = recipients_path.exists() && recipient::load(&recipients_path).is_ok();
    check(
        ".recipients valid",
        recipients_ok,
        &recipients_path.display().to_string(),
        &mut failures,
    );

    // Attempt to unlock so we can verify the identity actually matches one of
    // the recipients (otherwise reads would silently fail later).
    match commands::unlock(&config) {
        Ok(identity) => {
            check(
                "identity unlocks",
                true,
                "ok (via env / agent / prompt)",
                &mut failures,
            );

            if let Ok(list) = recipient::load(&recipients_path) {
                let own = identity.to_public();
                check(
                    "identity is in .recipients",
                    recipient::contains(&list, &own),
                    &own.to_string(),
                    &mut failures,
                );
            }
        }
        Err(e) => check("identity unlocks", false, &e.to_string(), &mut failures),
    }

    let session = agent::get(&config.store_dir).is_some();
    eprintln!(
        "  {} session cache: {}",
        if session {
            "●".green().to_string()
        } else {
            "○".dimmed().to_string()
        },
        if session { "active" } else { "not cached" }
    );

    if git::is_repo(&config.store_dir) {
        match git::status(&config.store_dir) {
            Ok(out) => {
                eprintln!("  {} git status:", "●".cyan());
                for line in out.lines() {
                    eprintln!("    {line}");
                }
            }
            Err(e) => check("git status", false, &e.to_string(), &mut failures),
        }
    } else {
        eprintln!("  {} git: not a repo", "○".dimmed());
    }

    if failures == 0 {
        eprintln!("\n{} all checks passed", "◆".green().bold());
        Ok(ExitCode::SUCCESS)
    } else {
        eprintln!(
            "\n{} {} check(s) failed",
            "✗".red().bold(),
            failures.to_string().bold()
        );
        Ok(ExitCode::from(1))
    }
}

fn check(label: &str, ok: bool, detail: &str, failures: &mut usize) {
    let mark = if ok {
        "◆".green().bold().to_string()
    } else {
        "✗".red().bold().to_string()
    };
    eprintln!("  {mark} {label}: {detail}");
    if !ok {
        *failures = failures.saturating_add(1);
    }
}
