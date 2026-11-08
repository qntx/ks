//! `ks otp <name> [-c]` — TOTP code.

use std::time::SystemTime;

use super::*;

/// Prints `code` and `valid_for`; `-c` copies the code.
pub fn run(cli: &Cli, out: &mut Out, n: &str, copy: bool) -> CliResult<i32> {
    let session = unlock(cli, out)?;
    let name = name(n)?;
    let entry = session.get(&name).map_err(|e| err(&e))?;
    let (code, valid_for) = ks::totp::code(&entry, SystemTime::now()).map_err(|e| err(&e))?;
    if copy {
        clipboard::copy(code.as_bytes())?;
        return Ok(out.ok_value(
            "copied",
            serde_json::json!({ "name": name.to_string(), "copied": true }),
        ));
    }
    Ok(out.ok_value(
        &format!("{code} (valid for {valid_for}s)"),
        serde_json::json!({ "name": name.to_string(), "code": code, "valid_for": valid_for }),
    ))
}
