//! `ks run -e VAR=name[#field] -- cmd...` — inject env vars and run.

use std::process::{Command, Stdio};

use ks::{ErrorCode, Kind};

use super::{Cli, CliResult, Out, err, fail, name, unlock};

/// Resolves each mapping, spawns with inherited stdio, passes the exit code.
pub(crate) fn run(cli: &Cli, out: &Out, env: &[String], command: &[String]) -> CliResult<i32> {
    if env.is_empty() {
        return fail(ErrorCode::Usage, "at least one -e VAR=name is required");
    }
    let session = unlock(cli, out)?;
    let mut vars: Vec<(String, zeroize::Zeroizing<String>)> = Vec::with_capacity(env.len());
    for spec in env {
        let (var, target) = spec.split_once('=').ok_or_else(|| {
            (
                ErrorCode::Usage,
                format!("invalid mapping {spec:?}, expected VAR=name[#field]"),
            )
        })?;
        if !var
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            || !var.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            return fail(ErrorCode::Usage, format!("invalid variable name {var:?}"));
        }
        let (entry_name, field) = target.split_once('#').unwrap_or((target, ""));
        let name = name(entry_name)?;
        let entry = session.get(&name).map_err(|e| err(&e))?;
        let value = if field.is_empty() {
            let text = entry
                .as_text()
                .ok_or_else(|| (ErrorCode::Usage, format!("{entry_name} is binary")))?;
            text.value().to_owned()
        } else {
            if entry.kind() == Kind::Binary {
                return fail(ErrorCode::Usage, format!("{entry_name} is binary"));
            }
            entry
                .as_text()
                .and_then(|t| t.field(field))
                .map(str::to_owned)
                .ok_or_else(|| {
                    err(&ks::Error::NotFound(format!(
                        "field {field:?} in {entry_name}"
                    )))
                })?
        };
        vars.push((var.to_owned(), zeroize::Zeroizing::new(value)));
    }
    let Some((program, args)) = command.split_first() else {
        return fail(ErrorCode::Usage, "no command given");
    };
    let mut cmd = Command::new(program);
    cmd.args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    for (k, v) in &vars {
        cmd.env(k, v.as_str());
    }
    let status = cmd
        .status()
        .map_err(|e| (ErrorCode::Unavailable, format!("cannot run {program}: {e}")))?;
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt as _;
        if let Some(sig) = status.signal() {
            return Ok(128 + sig);
        }
    }
    Ok(status.code().unwrap_or(1))
}
