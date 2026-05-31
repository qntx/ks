//! `ks run -- <cmd>` — execute a command with secrets injected as env vars.

use std::process::{Command as Proc, ExitCode};

use ks::{Config, Error, Result};
use zeroize::Zeroizing;

use crate::commands;
use crate::terminal;

pub fn run(config: &Config, env: &[String], prefix: &[String], cmd: &[String]) -> Result<ExitCode> {
    let (program, args) = cmd
        .split_first()
        .ok_or_else(|| Error::InvalidArgument("missing command after `--`".into()))?;

    let store = commands::open_store(config)?;
    let identity = commands::unlock(config)?;
    let mut injected: Vec<(String, Zeroizing<String>)> = Vec::new();

    for raw in env {
        let (path, name) = raw.split_once('=').ok_or_else(|| {
            Error::InvalidArgument(format!("expected `<path>=<NAME>`, got `{raw}`"))
        })?;
        let secret = store.get(path, &identity)?;
        injected.push((
            name.to_owned(),
            Zeroizing::new(secret.password().to_owned()),
        ));
    }

    for pfx in prefix {
        let paths = store.list(pfx)?;
        if paths.is_empty() {
            terminal::warn(&format!("no secrets under `{pfx}`"));
        }
        for path in paths {
            let secret = store.get(&path, &identity)?;
            let suffix = path
                .strip_prefix(pfx)
                .and_then(|s| s.strip_prefix('/'))
                .unwrap_or(&path);
            let env_name = suffix.replace(['/', '-'], "_").to_uppercase();
            injected.push((env_name, Zeroizing::new(secret.password().to_owned())));
        }
    }

    let mut child = Proc::new(program);
    child.args(args);
    for (name, value) in &injected {
        child.env(name, value.as_str());
    }

    let status = child.status().map_err(Error::Io)?;
    drop(injected);

    Ok(commands::child_exit_code(status))
}
