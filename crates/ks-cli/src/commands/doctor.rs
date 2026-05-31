//! `ks doctor` — sanity-check the store, identity, recipients and git state.

use std::io::IsTerminal as _;
use std::path::Path;
use std::process::ExitCode;

use ks::{Config, crypto, git, x25519};
use owo_colors::{OwoColorize as _, Stream, Style};

use crate::commands;

/// Accumulates check results so they can be rendered as human lines (printed
/// inline as they run) or a single JSON object at the end.
#[derive(Default)]
struct Report {
    checks: Vec<CheckLine>,
    notes: Vec<String>,
    failures: usize,
}

struct CheckLine {
    label: String,
    ok: bool,
    detail: String,
}

impl Report {
    fn check(&mut self, label: &str, ok: bool, detail: &str) {
        if !ok {
            self.failures = self.failures.saturating_add(1);
        }
        if !crate::output::is_json() {
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
        }
        self.checks.push(CheckLine {
            label: label.to_owned(),
            ok,
            detail: detail.to_owned(),
        });
    }

    fn note(&mut self, detail: &str) {
        if !crate::output::is_json() {
            eprintln!(
                "  {} {detail}",
                "[*]".if_supports_color(Stream::Stderr, |t| t.cyan()),
            );
        }
        self.notes.push(detail.to_owned());
    }
}

pub fn run(config: &Config) -> ExitCode {
    let mut report = Report::default();

    report.check(
        "identity file present",
        config.identity_path.exists(),
        &config.identity_path.display().to_string(),
    );
    report.check(
        "store directory present",
        config.store_dir.exists(),
        &config.store_dir.display().to_string(),
    );

    let recipients_path = config.recipients_path();
    let recipients_ok =
        recipients_path.exists() && crypto::load_recipients(&recipients_path).is_ok();
    report.check(
        ".age-recipients valid",
        recipients_ok,
        &recipients_path.display().to_string(),
    );

    check_permissions(config, &mut report);

    let identity = check_identity(config, &recipients_path, &mut report);
    if let Some(identity) = &identity {
        check_secrets(config, identity, &mut report);
    }

    check_runtime_artifacts(config, &mut report);
    report_git(config, &mut report);

    finish(&report)
}

/// Emits the final summary (JSON object, or a coloured pass/fail line) and maps
/// to the process exit code.
fn finish(report: &Report) -> ExitCode {
    let ok = report.failures == 0;
    if crate::output::is_json() {
        let checks: Vec<serde_json::Value> = report
            .checks
            .iter()
            .map(|line| {
                serde_json::json!({ "check": line.label, "ok": line.ok, "detail": line.detail })
            })
            .collect();
        crate::output::emit(&serde_json::json!({
            "checks": checks,
            "notes": report.notes,
            "failures": report.failures,
            "ok": ok,
        }));
    } else if ok {
        eprintln!(
            "\n{} all checks passed",
            "[OK]".if_supports_color(Stream::Stderr, |t| t.style(Style::new().green().bold())),
        );
    } else {
        eprintln!(
            "\n{} {} check(s) failed",
            "[FAIL]".if_supports_color(Stream::Stderr, |t| t.style(Style::new().red().bold())),
            report
                .failures
                .to_string()
                .if_supports_color(Stream::Stderr, |t| t.bold()),
        );
    }
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

/// Records git repo status as a note (human prints the full `git status -sb`).
fn report_git(config: &Config, report: &mut Report) {
    if !git::is_repo(&config.store_dir) {
        report.note("git: not a repository");
        return;
    }
    match git::status(&config.store_dir) {
        Ok(out) => {
            if crate::output::is_json() {
                let branch = out.lines().next().unwrap_or("").trim().to_owned();
                report.notes.push(format!("git {branch}"));
            } else {
                eprintln!(
                    "  {} git status:",
                    "[*]".if_supports_color(Stream::Stderr, |t| t.cyan()),
                );
                for line in out.lines() {
                    eprintln!("    {line}");
                }
            }
        }
        Err(e) => report.check("git status", false, &e.to_string()),
    }
}

/// Verifies the identity unlocks and is present in the recipient list.
///
/// Skipped when neither `KS_PASSPHRASE` nor an interactive terminal is
/// available, so `ks doctor` stays non-blocking in scripts and CI.
fn check_identity(
    config: &Config,
    recipients_path: &Path,
    report: &mut Report,
) -> Option<x25519::Identity> {
    let can_unlock = std::env::var("KS_PASSPHRASE").is_ok_and(|v| !v.is_empty())
        || std::io::stdin().is_terminal();
    if !can_unlock {
        report.note("identity unlocks: skipped (set KS_PASSPHRASE for non-interactive checks)");
        return None;
    }
    let identity = match commands::unlock(config) {
        Ok(id) => id,
        Err(e) => {
            report.check("identity unlocks", false, &e.to_string());
            return None;
        }
    };
    report.check("identity unlocks", true, "ok (env or prompt)");
    if let Ok(list) = crypto::load_recipients(recipients_path) {
        let own = identity.to_public();
        report.check(
            "identity is in .age-recipients",
            crypto::recipients_contain(&list, &own),
            &own.to_string(),
        );
    }
    Some(identity)
}

/// Flags any identity/store/recipients path readable by group or other (Unix).
fn check_permissions(config: &Config, report: &mut Report) {
    let issues = config.permission_issues();
    if issues.is_empty() {
        report.check("file permissions owner-only", true, "ok");
    } else {
        for issue in &issues {
            report.check("file permissions", false, issue);
        }
    }
}

/// Spot-checks that a sample of secrets decrypt and pass envelope verification,
/// catching tampering, relocation, or legacy (pre-envelope) files.
fn check_secrets(config: &Config, identity: &x25519::Identity, report: &mut Report) {
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
    report.check(
        &format!("secrets decrypt & verify ({checked} sampled)"),
        bad == 0,
        &if bad == 0 {
            "ok".to_owned()
        } else {
            format!("{bad} failed integrity/decrypt")
        },
    );
}

/// Reports leftover runtime artifacts (interrupted rotation, scratch files) as
/// non-fatal notes with a cleanup hint.
fn check_runtime_artifacts(config: &Config, report: &mut Report) {
    let staging = config.store_dir.join(".ks-rotate");
    if staging.exists() {
        report.note(&format!(
            "leftover rotation staging at {} — safe to delete (a rotation was interrupted)",
            staging.display()
        ));
    }
    let temps = orphan_temp_count(&config.store_dir);
    if temps > 0 {
        report.note(&format!(
            "{temps} leftover *.tmp scratch file(s) under the store — safe to delete"
        ));
    }
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
