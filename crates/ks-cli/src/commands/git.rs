//! `ks git <args>...` — run git inside the vault with stdio inherited.

use std::process::{Command, Stdio};

use ks::ErrorCode;

use super::{fail, vault, Cli, CliResult, Out};

/// Passes `args` to `git -C <vault>`; the exit code is the child's.
pub(crate) fn run(cli: &Cli, out: &Out, args: &[String]) -> CliResult<i32> {
    let v = vault(cli)?;
    let Some((sub, rest)) = args.split_first() else {
        return fail(ErrorCode::Usage, "no git arguments given");
    };
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(v.root()).arg(sub).args(rest)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    if !out.interactive {
        cmd.env("GIT_TERMINAL_PROMPT", "0");
    }
    let status = cmd
        .status()
        .map_err(|e| (ErrorCode::Unavailable, format!("cannot run git: {e}")))?;
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt as _;
        if let Some(sig) = status.signal() {
            return Ok(128 + sig);
        }
    }
    Ok(status.code().unwrap_or(1))
}
