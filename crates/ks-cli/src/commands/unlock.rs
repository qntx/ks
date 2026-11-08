//! `ks unlock [--ttl] [--max]` — hand the vault key to a session agent.

use std::io::{BufRead as _, BufReader, Write as _};
use std::process::{Command, Stdio};

use ks::ErrorCode;

use super::{Cli, CliResult, Out, err, fail, prompt, vault};

const MAX_LIFE: u64 = 24 * 3600;

/// Parses `<u64>(s|m|h)` into seconds.
pub(crate) fn parse_duration(text: &str) -> CliResult<u64> {
    let (num, mult) = [('s', 1_u64), ('m', 60), ('h', 3600)]
        .iter()
        .find_map(|(suffix, mult)| text.strip_suffix(*suffix).map(|n| (n, *mult)))
        .unwrap_or((text, 1));
    let secs = num
        .parse::<u64>()
        .ok()
        .and_then(|n| n.checked_mul(mult))
        .ok_or_else(|| (ErrorCode::Usage, format!("bad duration {text:?}")))?;
    Ok(secs)
}

/// Unlocks without the agent, then spawns `ks __agent` with the identity
/// text over its stdin pipe and waits up to 5 s for `ready`.
pub(crate) fn run(cli: &Cli, out: &Out, ttl: &str, max: &str) -> CliResult<i32> {
    let ttl = parse_duration(ttl)?;
    let max = parse_duration(max)?;
    if ttl < 1 {
        return fail(ErrorCode::Usage, "--ttl must be at least 1s");
    }
    if max > MAX_LIFE {
        return fail(ErrorCode::Usage, "--max may not exceed 24h");
    }
    if ttl > max {
        return fail(ErrorCode::Usage, "--ttl may not exceed --max");
    }

    let v = vault(cli)?;
    let source = prompt::passphrase_source(cli, out);
    let session = v.unlock_without_agent(Some(&source)).map_err(|e| err(&e))?;
    for w in session.warnings() {
        out.warn(w.clone());
    }
    let Some(identity) = session.agent_identity() else {
        return fail(ErrorCode::Internal, "vault has no exportable identity");
    };

    // An already-running agent gets a stale key; ask it to exit first.
    drop(ks::agent::lock(&v));

    let exe = std::env::current_exe()
        .map_err(|e| (ErrorCode::Internal, format!("cannot find ks binary: {e}")))?;
    let mut cmd = Command::new(exe);
    cmd.arg("--vault")
        .arg(v.root())
        .arg("__agent")
        .arg("--ttl")
        .arg(ttl.to_string())
        .arg("--max")
        .arg(max.to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .env("GIT_TERMINAL_PROMPT", "0");
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut cmd, 0);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        cmd.creation_flags(0x0000_0008 | 0x0800_0000); // DETACHED_PROCESS | CREATE_NO_WINDOW
    }
    crate::harden::no_inherit_std_handles();
    let mut child = cmd.spawn().map_err(|e| {
        (
            ErrorCode::Unavailable,
            format!("cannot spawn ks __agent: {e}"),
        )
    })?;

    // Identity to stdin, then close it so the daemon sees EOF.
    {
        use age::secrecy::ExposeSecret as _;
        let Some(mut stdin) = child.stdin.take() else {
            return fail(ErrorCode::Internal, "agent stdin pipe missing");
        };
        if stdin
            .write_all(identity.expose_secret().as_bytes())
            .is_err()
            || stdin.write_all(b"\n").is_err()
        {
            drop(child.kill());
            return fail(ErrorCode::Unavailable, "failed to feed the agent");
        }
    }
    drop(child.stdin.take());

    let Some(stdout) = child.stdout.take() else {
        return fail(ErrorCode::Internal, "agent stdout pipe missing");
    };
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        let mut reader = BufReader::new(stdout);
        drop(reader.read_line(&mut line));
        if tx.send(line.trim().eq_ignore_ascii_case("ready")).is_err() {}
    });
    if rx.recv_timeout(std::time::Duration::from_secs(5)) == Ok(true) {
        Ok(out.ok_value(
            "unlocked into the session agent",
            serde_json::json!({ "unlocked": true, "ttl": ttl, "max": max }),
        ))
    } else {
        drop(child.kill());
        fail(
            ErrorCode::Unavailable,
            "agent did not become ready within 5s",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::parse_duration;

    #[test]
    fn durations() {
        assert_eq!(parse_duration("15m").unwrap_or(0), 900);
        assert_eq!(parse_duration("300s").unwrap_or(0), 300);
        assert_eq!(parse_duration("8h").unwrap_or(0), 28_800);
        assert_eq!(parse_duration("60").unwrap_or(0), 60);
        assert!(parse_duration("x").is_err());
        assert!(parse_duration("").is_err());
        assert!(parse_duration("-1s").is_err());
        assert!(parse_duration("18446744073709551615h").is_err());
    }
}
