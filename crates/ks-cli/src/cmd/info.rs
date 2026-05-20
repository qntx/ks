use std::time::{Duration, UNIX_EPOCH};

use owo_colors::OwoColorize as _;

pub fn run(config: &ks::Config, path: &str) -> ks::Result<()> {
    let vault = super::open_vault(config)?;
    let secret = vault.get(path)?;

    let fmt_ts = |ts: u64| {
        let dt = UNIX_EPOCH + Duration::from_secs(ts);
        let secs = dt.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        format!("{secs}")
    };

    eprintln!("{}", path.bold());
    eprintln!("  {} {}", "created:".dimmed(), fmt_ts(secret.created_at));
    eprintln!("  {} {}", "updated:".dimmed(), fmt_ts(secret.updated_at));

    if !secret.note.is_empty() {
        eprintln!("  {} {}", "note:".dimmed(), secret.note);
    }

    if !secret.fields.is_empty() {
        eprintln!("  {}:", "fields".dimmed());
        let mut keys: Vec<&String> = secret.fields.keys().collect();
        keys.sort();
        for k in keys {
            eprintln!("    {} = •••••••", k.cyan());
        }
    }
    Ok(())
}
