//! `ks show` — print, copy, or inspect a secret.

use std::process::ExitCode;

use ks::{Config, Error, Result, Secret};
use owo_colors::{OwoColorize as _, Stream};

use crate::clipboard;
use crate::commands;
use crate::terminal;

pub fn run(
    config: &Config,
    path: &str,
    copy: bool,
    field: Option<&str>,
    meta: bool,
) -> Result<ExitCode> {
    let store = commands::open_store(config)?;
    let identity = commands::unlock(config)?;
    let secret = store.get(path, &identity)?;

    if meta {
        print_meta(path, &secret);
        return Ok(ExitCode::SUCCESS);
    }

    if secret.is_binary() {
        return show_binary(path, &secret, copy, field);
    }

    let value = match field {
        Some(name) => secret
            .get(name)
            .ok_or_else(|| Error::SecretNotFound(format!("{path}#{name}")))?,
        // Copying the whole multi-line file is rarely useful, so copy targets the
        // primary value; printing shows the entire secret like `pass show`.
        None if copy => secret.password(),
        None => secret.expose(),
    };

    if copy {
        let secs = clipboard::copy_with_autoclear(value, commands::clip_secs())?;
        let what = field.map_or_else(|| path.to_owned(), |f| format!("{path}#{f}"));
        terminal::info(&format!("Copied {what} to clipboard (clears in {secs}s)"));
    } else {
        println!("{}", value.trim_end_matches('\n'));
    }
    Ok(ExitCode::SUCCESS)
}

/// Writes a binary secret's raw bytes to stdout. `--field` and `--copy` do not
/// apply to opaque binary payloads, so they are rejected as usage errors.
fn show_binary(path: &str, secret: &Secret, copy: bool, field: Option<&str>) -> Result<ExitCode> {
    use std::io::Write as _;
    if field.is_some() || copy {
        return Err(Error::InvalidArgument(format!(
            "{path} is a binary secret; --field and --copy do not apply"
        )));
    }
    std::io::stdout()
        .write_all(secret.as_bytes())
        .map_err(Error::Io)?;
    Ok(ExitCode::SUCCESS)
}

fn print_meta(path: &str, secret: &Secret) {
    eprintln!("{}", path.if_supports_color(Stream::Stderr, |t| t.bold()));
    let keys = secret.keys();
    if keys.is_empty() {
        eprintln!(
            "  {}",
            "(no fields)".if_supports_color(Stream::Stderr, |t| t.dimmed()),
        );
    } else {
        eprintln!(
            "  {}:",
            "fields".if_supports_color(Stream::Stderr, |t| t.dimmed()),
        );
        for key in keys {
            eprintln!(
                "    {} = {}",
                key.if_supports_color(Stream::Stderr, |t| t.cyan()),
                "•••••••".if_supports_color(Stream::Stderr, |t| t.dimmed()),
            );
        }
    }
}
