//! `ks env` --emit shell `export` statements for one or more secrets.

use std::process::ExitCode;

use ks::{Config, Error, Result};

use crate::commands;
use crate::terminal;

pub fn run(config: &Config, targets: &[String], shell: &str) -> Result<ExitCode> {
    let store = commands::open_store(config)?;
    let mut paths: Vec<String> = Vec::new();

    if targets.is_empty() {
        paths.extend(store.list("")?);
    } else {
        for t in targets {
            let bucket = store.list(t)?;
            if bucket.is_empty() && store.exists(t) {
                paths.push(t.clone());
            } else {
                paths.extend(bucket);
            }
        }
    }
    if paths.is_empty() {
        terminal::warn("No secrets matched");
        return Ok(ExitCode::SUCCESS);
    }

    let dialect = Dialect::parse(shell)?;
    for path in paths {
        let secret = store.get(&path)?;
        // Replace both `/` (separator) and `-` (allowed in paths but not in
        // POSIX shell identifiers) with `_`, then upper-case.
        let name = path.replace(['/', '-'], "_").to_uppercase();
        let value = secret.value.as_str();
        match dialect {
            Dialect::Sh => println!("export {name}={}", shell_quote_posix(value)),
            Dialect::Fish => println!("set -gx {name} {}", shell_quote_posix(value)),
            Dialect::Pwsh => println!("$env:{name} = {}", powershell_quote(value)),
        }
    }
    Ok(ExitCode::SUCCESS)
}

#[derive(Debug, Clone, Copy)]
enum Dialect {
    Sh,
    Fish,
    Pwsh,
}

impl Dialect {
    fn parse(s: &str) -> Result<Self> {
        match s.to_ascii_lowercase().as_str() {
            "sh" | "bash" | "zsh" => Ok(Self::Sh),
            "fish" => Ok(Self::Fish),
            "pwsh" | "powershell" | "ps" => Ok(Self::Pwsh),
            other => Err(Error::InvalidPath(format!(
                "unknown shell `{other}` (expected: sh, fish, pwsh)"
            ))),
        }
    }
}

fn shell_quote_posix(s: &str) -> String {
    let escaped: String = s
        .chars()
        .map(|c| {
            if c == '\'' {
                "'\\''".to_owned()
            } else {
                c.to_string()
            }
        })
        .collect();
    format!("'{escaped}'")
}

fn powershell_quote(s: &str) -> String {
    let escaped = s.replace('"', "`\"");
    format!("\"{escaped}\"")
}
