//! `ks info` -- show metadata for a secret without revealing the value.

use std::process::ExitCode;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ks::{Config, Kind, Result};
use owo_colors::OwoColorize as _;

use crate::commands;

pub fn run(config: &Config, path: &str) -> Result<ExitCode> {
    let store = commands::open_store(config)?;
    let secret = store.get(path)?;

    eprintln!("{}", path.bold());
    eprintln!(
        "  {}    {}",
        "kind".dimmed(),
        match secret.kind {
            Kind::Secret => "secret",
            Kind::Totp => "totp",
        }
    );
    eprintln!("  {} {}", "created".dimmed(), format_ts(secret.created_at));
    eprintln!("  {} {}", "updated".dimmed(), format_ts(secret.updated_at));

    if !secret.note.is_empty() {
        eprintln!("  {}    {}", "note".dimmed(), secret.note);
    }
    if !secret.fields.is_empty() {
        eprintln!("  {}:", "fields".dimmed());
        for key in secret.fields.keys() {
            eprintln!("    {} = {}", key.cyan(), "*******".dimmed());
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn format_ts(secs: u64) -> String {
    let dt = UNIX_EPOCH + Duration::from_secs(secs);
    let now = SystemTime::now();
    let delta = now.duration_since(dt).unwrap_or_default().as_secs();
    let human = if delta < 60 {
        format!("{delta}s ago")
    } else if delta < 3600 {
        format!("{}m ago", delta / 60)
    } else if delta < 86_400 {
        format!("{}h ago", delta / 3600)
    } else {
        format!("{}d ago", delta / 86_400)
    };
    format!("{secs} ({human})")
}
