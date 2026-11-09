//! `ks status` — vault, agent and git state without unlocking.

use std::process::Command;

use super::{Cli, CliResult, Out, vault};

/// Reports the vault path, agent liveness and git repo/dirty state.
pub(crate) fn run(cli: &Cli, out: &Out) -> CliResult<i32> {
    let v = vault(cli)?;
    let mut warnings = Vec::new();
    let agent = ks::agent::status(&v).unwrap_or_else(|e| {
        warnings.push(format!("could not read the agent state: {e}"));
        None
    });
    let git = ks::git::Git::new(v.root(), false);
    let repo = git.is_repo();
    let dirty = repo
        && Command::new("git")
            .arg("-C")
            .arg(v.root())
            .args(["status", "--porcelain"])
            .env("GIT_TERMINAL_PROMPT", "0")
            .output()
            .is_ok_and(|o| !o.stdout.is_empty());
    let touchid = ks::touchid::is_enabled(&v).unwrap_or(false);
    let keychain = ks::keychain::is_enabled(&v).unwrap_or(false);
    let value = serde_json::json!({
        "vault": v.root().display().to_string(),
        "agent": agent.as_ref().map(|a| serde_json::json!({
            "ttl_left": a.ttl_left, "life_left": a.life_left, "host": a.host,
        })),
        "touchid": touchid,
        "keychain": keychain,
        "git": { "repo": repo, "dirty": dirty },
        "warnings": warnings,
    });
    let agent_line = agent.map_or_else(
        || "agent: not running".to_owned(),
        |a| {
            format!(
                "agent: ttl {}s, life {}s, host {}",
                a.ttl_left, a.life_left, a.host
            )
        },
    );
    Ok(out.ok_value(
        &format!(
            "vault: {}\n{agent_line}\ntouchid: {}\nkeychain: {}\ngit: {}",
            v.root().display(),
            if touchid { "enabled" } else { "disabled" },
            if keychain { "enabled" } else { "disabled" },
            if repo {
                format!("repo, {}", if dirty { "dirty" } else { "clean" })
            } else {
                "not a repo".to_owned()
            }
        ),
        value,
    ))
}
