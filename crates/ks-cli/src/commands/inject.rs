//! `ks inject` --substitute `${KS:path}` and `${KS:path:field}` markers in a template.

use std::io::{Read as _, Write as _};
use std::path::Path;
use std::process::ExitCode;

use ks::{Config, Error, Result};
use zeroize::Zeroizing;

use crate::commands;
use crate::terminal;

const OPEN: &str = "${KS:";
const CLOSE: char = '}';

pub fn run(config: &Config, input: Option<&Path>, output: Option<&Path>) -> Result<ExitCode> {
    let template = read_input(input)?;
    let store = commands::open_store(config)?;

    let mut buf = String::with_capacity(template.len());
    let mut remaining = template.as_str();
    let mut count: usize = 0;

    while let Some(start) = remaining.find(OPEN) {
        buf.push_str(&remaining[..start]);
        let after_open = &remaining[start + OPEN.len()..];
        let end = after_open
            .find(CLOSE)
            .ok_or_else(|| Error::InvalidPath("unterminated `${KS:...}` marker".into()))?;
        let key = &after_open[..end];
        let (path, field) = key
            .split_once(':')
            .map_or((key, None), |(p, f)| (p, Some(f)));

        let secret = store.get(path)?;
        match field {
            None => buf.push_str(secret.value.as_str()),
            Some(name) => {
                let val = secret
                    .field(name)
                    .ok_or_else(|| Error::SecretNotFound(format!("{path}#{name}")))?;
                buf.push_str(val);
            }
        }
        remaining = &after_open[end + 1..];
        count = count.saturating_add(1);
    }
    buf.push_str(remaining);

    let owned = Zeroizing::new(buf);
    write_output(output, owned.as_str())?;
    terminal::info(&format!("Substituted {count} marker(s)"));
    Ok(ExitCode::SUCCESS)
}

fn read_input(path: Option<&Path>) -> Result<String> {
    match path {
        Some(p) if p != Path::new("-") => std::fs::read_to_string(p).map_err(Error::Io),
        _ => {
            let mut buf = String::new();
            std::io::stdin()
                .read_to_string(&mut buf)
                .map_err(Error::Io)?;
            Ok(buf)
        }
    }
}

fn write_output(path: Option<&Path>, content: &str) -> Result<()> {
    match path {
        Some(p) if p != Path::new("-") => std::fs::write(p, content).map_err(Error::Io),
        _ => {
            let stdout = std::io::stdout();
            let mut handle = stdout.lock();
            handle.write_all(content.as_bytes()).map_err(Error::Io)
        }
    }
}
