//! `ks doctor` — sanity-check the store, identity, recipients and git state.

use std::io::IsTerminal as _;
use std::path::Path;
use std::process::ExitCode;

use ks::{Config, crypto, git, x25519};
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

    check_permissions(config, &mut failures);

    let identity = check_identity(config, &recipients_path, &mut failures);
    if let Some(identity) = &identity {
        check_secrets(config, identity, &mut failures);
    }

    check_runtime_artifacts(config);

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
fn check_identity(
    config: &Config,
    recipients_path: &Path,
    failures: &mut usize,
) -> Option<x25519::Identity> {
    let can_unlock = std::env::var("KS_PASSPHRASE").is_ok_and(|v| !v.is_empty())
        || std::io::stdin().is_terminal();
    if !can_unlock {
        eprintln!(
            "  {} identity unlocks: skipped (set KS_PASSPHRASE for non-interactive checks)",
            "[--]".if_supports_color(Stream::Stderr, |t| t.dimmed()),
        );
        return None;
    }
    let identity = match commands::unlock(config) {
        Ok(id) => id,
        Err(e) => {
            check("identity unlocks", false, &e.to_string(), failures);
            return None;
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
    Some(identity)
}

/// Flags any identity/store/recipients path readable by group or other (Unix).
fn check_permissions(config: &Config, failures: &mut usize) {
    let issues = config.permission_issues();
    if issues.is_empty() {
        check("file permissions owner-only", true, "ok", failures);
    } else {
        for issue in &issues {
            check("file permissions", false, issue, failures);
        }
    }
}

/// Spot-checks that a sample of secrets decrypt and pass envelope verification,
/// catching tampering, relocation, or legacy (pre-envelope) files.
fn check_secrets(config: &Config, identity: &x25519::Identity, failures: &mut usize) {
    const SAMPLE: usize = 20;
    let Ok(store) = commands::open_store(config) else {
        return;
    };
    let Ok(paths) = store.list("") else {
        return;
    };
    if paths.is_empty() {
        return;
    }
    let checked = paths.len().min(SAMPLE);
    let bad = paths
        .iter()
        .take(SAMPLE)
        .filter(|path| store.get(path, identity).is_err())
        .count();
    check(
        &format!("secrets decrypt & verify ({checked} sampled)"),
        bad == 0,
        &if bad == 0 {
            "ok".to_owned()
        } else {
            format!("{bad} failed integrity/decrypt")
        },
        failures,
    );
}

/// Reports leftover runtime artifacts (interrupted rotation, scratch files) as
/// non-fatal notes with a cleanup hint.
fn check_runtime_artifacts(config: &Config) {
    let staging = config.store_dir.join(".ks-rotate");
    if staging.exists() {
        note(&format!(
            "leftover rotation staging at {} — safe to delete (a rotation was interrupted)",
            staging.display()
        ));
    }
    let temps = orphan_temp_count(&config.store_dir);
    if temps > 0 {
        note(&format!(
            "{temps} leftover *.tmp scratch file(s) under the store — safe to delete"
        ));
    }
}

fn note(detail: &str) {
    eprintln!(
        "  {} {detail}",
        "[*]".if_supports_color(Stream::Stderr, |t| t.cyan()),
    );
}

/// Counts `*.tmp` scratch files left by an interrupted atomic write, skipping
/// dot-directories (`.git`, `.ks-rotate`, …).
fn orphan_temp_count(root: &Path) -> usize {
    let mut count = 0;
    count_temps(root, &mut count);
    count
}

fn count_temps(dir: &Path, count: &mut usize) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_dir = path.is_dir();
        let is_hidden = entry.file_name().to_string_lossy().starts_with('.');
        if is_dir && !is_hidden {
            count_temps(&path, count);
        } else if !is_dir && path.extension().and_then(|e| e.to_str()) == Some("tmp") {
            *count = count.saturating_add(1);
        }
    }
}
