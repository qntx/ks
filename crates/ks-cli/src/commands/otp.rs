//! `ks otp` — generate a TOTP code from a secret's `otpauth://` source.

use std::process::ExitCode;

use ks::{Config, Error, Result, totp};

use crate::clipboard;
use crate::commands;
use crate::terminal;

pub fn run(config: &Config, path: &str, copy: bool) -> Result<ExitCode> {
    let store = commands::open_store(config)?;
    let identity = commands::unlock(config)?;
    let secret = store.get(path, &identity)?;

    let source = secret
        .otp()
        .ok_or_else(|| Error::InvalidTotp(format!("no TOTP source in {path}")))?;
    let code = totp::current(source)?;

    if copy {
        let secs = clipboard::copy_with_autoclear(&code.value, commands::clip_secs())?;
        terminal::info(&format!(
            "Copied OTP (valid {}s, clears in {secs}s)",
            code.remaining_secs
        ));
    } else {
        println!("{}", code.value);
        terminal::info(&format!("valid for {}s", code.remaining_secs));
    }
    Ok(ExitCode::SUCCESS)
}
