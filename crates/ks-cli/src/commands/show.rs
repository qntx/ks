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

    if crate::output::is_json() {
        return show_json(path, &secret, field, meta);
    }

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

/// Builds the `--json` representation of a secret. `--meta` lists field names
/// only (no values); `--field` returns one field; binary secrets are base64;
/// otherwise the primary value, full text, and parsed fields are returned.
fn show_json(path: &str, secret: &Secret, field: Option<&str>, meta: bool) -> Result<ExitCode> {
    let value = if meta {
        serde_json::json!({ "path": path, "fields": secret.keys() })
    } else if let Some(name) = field {
        let field_value = secret
            .get(name)
            .ok_or_else(|| Error::SecretNotFound(format!("{path}#{name}")))?;
        serde_json::json!({ "path": path, "field": name, "value": field_value })
    } else if secret.is_binary() {
        use base64::Engine as _;
        let b64 = base64::engine::general_purpose::STANDARD.encode(secret.as_bytes());
        serde_json::json!({ "path": path, "kind": "binary", "base64": b64 })
    } else {
        let fields: serde_json::Map<String, serde_json::Value> = secret
            .fields()
            .map(|(k, v)| (k.to_owned(), serde_json::Value::String(v.to_owned())))
            .collect();
        serde_json::json!({
            "path": path,
            "kind": "text",
            "value": secret.password(),
            "text": secret.expose(),
            "fields": fields,
        })
    };
    crate::output::emit(&value);
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
