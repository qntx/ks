//! `ks otp` --generate the current TOTP code for a secret.

use std::process::ExitCode;

use ks::{Config, Result, totp};

use crate::clipboard;
use crate::commands;
use crate::terminal;

pub fn run(config: &Config, path: &str, copy: bool) -> Result<ExitCode> {
    let clear_secs = config.tunables.clipboard_clear_secs;
    let store = commands::open_store(config)?;
    let secret = store.get(path)?;
    let code = totp::current(secret.value.as_str())?;

    if copy {
        let secs = clipboard::copy_with_autoclear(&code.value, clear_secs)?;
        terminal::info(&format!(
            "Copied OTP for {path} (clears in {secs}s; code valid {} more seconds)",
            code.remaining_secs
        ));
    } else {
        println!("{}", code.value);
        terminal::info(&format!(
            "valid for {}s more (step {}s)",
            code.remaining_secs, code.step_secs
        ));
    }
    Ok(ExitCode::SUCCESS)
}
