//! `ks run -- <cmd>` --execute a command with secrets injected into its environment.

use std::process::{Command, ExitCode};

use ks::{Config, Error, Result};
use zeroize::Zeroizing;

use crate::commands;
use crate::terminal;

pub fn run(config: &Config, env: &[String], prefix: &[String], cmd: &[String]) -> Result<ExitCode> {
    let (program, args) = cmd
        .split_first()
        .ok_or_else(|| Error::InvalidPath("missing command after `--`".into()))?;

    let store = commands::open_store(config)?;
    let mut injected: Vec<(String, Zeroizing<String>)> = Vec::new();

    for raw in env {
        let (path, name) = raw
            .split_once('=')
            .ok_or_else(|| Error::InvalidPath(format!("expected `<path>=<NAME>`, got `{raw}`")))?;
        let secret = store.get(path)?;
        injected.push((name.to_owned(), Zeroizing::new(secret.value.to_string())));
    }

    for pfx in prefix {
        let paths = store.list(pfx)?;
        if paths.is_empty() {
            terminal::warn(&format!("no secrets under `{pfx}`"));
        }
        for path in paths {
            let secret = store.get(&path)?;
            let suffix = path
                .strip_prefix(pfx)
                .and_then(|s| s.strip_prefix('/'))
                .unwrap_or(&path);
            let env_name = suffix.replace(['/', '-'], "_").to_uppercase();
            injected.push((env_name, Zeroizing::new(secret.value.to_string())));
        }
    }

    let mut child = Command::new(program);
    child.args(args);
    for (name, value) in &injected {
        child.env(name, value.as_str());
    }

    let status = child.status().map_err(Error::Io)?;
    // Explicitly drop the zeroizing wrappers before we exit.
    drop(injected);

    let code = u8::try_from(status.code().unwrap_or(1).clamp(0, 255)).unwrap_or(1);
    Ok(ExitCode::from(code))
}
