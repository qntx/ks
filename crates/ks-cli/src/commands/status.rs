//! `ks status` — vault, agent and git state without unlocking.

use std::process::Command;

use super::{Cli, CliResult, Out, err, vault};

/// Reports the vault path, agent liveness and git repo/dirty state.
pub(crate) fn run(cli: &Cli, out: &Out) -> CliResult<i32> {
    let v = vault(cli)?;
    let agent = ks::agent::status(&v).map_err(|e| err(&e))?;
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
    let value = serde_json::json!({
        "vault": v.root().display().to_string(),
        "agent": agent.map(|a| serde_json::json!({
            "ttl_left": a.ttl_left, "life_left": a.life_left,
        })),
        "git": { "repo": repo, "dirty": dirty },
    });
    let agent_line = agent.map_or_else(
        || "agent: not running".to_owned(),
        |a| format!("agent: ttl {}s, life {}s", a.ttl_left, a.life_left),
    );
    Ok(out.ok_value(
        &format!(
            "vault: {}\n{agent_line}\ngit: {}",
            v.root().display(),
            if repo {
                format!("repo, {}", if dirty { "dirty" } else { "clean" })
            } else {
                "not a repo".to_owned()
            }
        ),
        value,
    ))
}
