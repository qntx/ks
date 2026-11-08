//! `ks set <name> [-m|-b] [--force]` — store a secret.

use ks::{Entry, ErrorCode};
use zeroize::Zeroizing;

use super::*;

/// Reads the secret and stores it.
pub fn run(
    cli: &Cli,
    out: &mut Out,
    n: &str,
    multiline: bool,
    binary: bool,
    force: bool,
) -> CliResult<i32> {
    if multiline && binary {
        return fail(ErrorCode::Usage, "-m and -b are mutually exclusive");
    }
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
    let entry = if binary {
        let bytes = prompt::stdin_all()?;
        if bytes.is_empty() {
            return fail(ErrorCode::Usage, "empty input on stdin");
        }
        Entry::binary(bytes).map_err(|e| err(&e))?
    } else {
        let text = if multiline {
            let raw = prompt::stdin_all()?;
            let s = String::from_utf8(raw.as_slice().to_vec())
                .map_err(|_| (ErrorCode::Usage, "stdin is not UTF-8".to_owned()))?;
            Zeroizing::new(s.replace("\r\n", "\n"))
        } else if out.interactive {
            let input = cliclack::password("value")
                .interact()
                .map_err(|_| (ErrorCode::Cancelled, "cancelled".to_owned()))?;
            Zeroizing::new(input)
        } else {
            prompt::stdin_first_line()?
        };
        if text.is_empty() {
            return fail(ErrorCode::Usage, "empty value");
        }
        Entry::text(text).map_err(|e| err(&e))?
    };
    session.set(&name, &entry, true).map_err(|e| err(&e))?;
    Ok(out.ok_value(
        &format!("stored {name}"),
        serde_json::json!({ "name": name.to_string(), "stored": true }),
    ))
}
