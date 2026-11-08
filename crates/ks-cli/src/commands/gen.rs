//! `ks gen [<name>] [-l len] [-s charset] [-c] [--force]`.

use ks::pwgen::{Charset, generate};
use ks::{Entry, ErrorCode};
use zeroize::Zeroizing;

use super::{Cli, CliResult, Out, autocommit, clipboard, err, fail, name, prompt, unlock};

/// Generates a password; prints it, stores it under `name`, or copies it.
pub(crate) fn run(
    cli: &Cli,
    out: &Out,
    n: Option<&str>,
    length: usize,
    charset: &str,
    copy: bool,
    force: bool,
) -> CliResult<i32> {
    let charset = match charset {
        "alphanum" => Charset::Alphanum,
        "hex" => Charset::Hex,
        "printable" => Charset::Printable,
        "slug" => Charset::Slug,
        other => return fail(ErrorCode::Usage, format!("unknown charset {other:?}")),
    };
    let value = generate(length, charset).map_err(|e| err(&e))?;

    if copy {
        clipboard::copy(value.as_bytes())?;
        return Ok(out.ok_value("copied", serde_json::json!({ "copied": true })));
    }

    match n {
        None => Ok(out.ok_value(&value, serde_json::json!({ "value": &*value }))),
        Some(n) => {
            let session = unlock(cli, out)?;
            let name = name(n)?;
            let exists = name.path_in(session.vault().root()).exists();
            if exists && !force {
                if out.interactive {
                    prompt::confirm(out, &format!("overwrite {name}?"))?;
                } else {
                    return Err(err(&ks::Error::Exists(name.to_string())));
                }
            }
            let entry = Entry::text(Zeroizing::new(value.to_string())).map_err(|e| err(&e))?;
            session.set(&name, &entry, true).map_err(|e| err(&e))?;
            autocommit(
                cli,
                out,
                &[name.path_in(session.vault().root())],
                &format!("ks: set {name}"),
            );
            Ok(out.ok_value(
                &format!("stored {name}"),
                serde_json::json!({ "name": name.to_string(), "stored": true }),
            ))
        }
    }
}
