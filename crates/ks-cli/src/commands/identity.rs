//! `ks identity` — print this device's public recipient.

use std::process::ExitCode;

use ks::{Config, Result};

use crate::commands;

pub fn run(config: &Config) -> Result<ExitCode> {
    let identity = commands::unlock(config)?;
    let public = identity.to_public().to_string();
    if crate::output::is_json() {
        crate::output::emit(&serde_json::json!({ "public_key": public }));
    } else {
        println!("{public}");
    }
    Ok(ExitCode::SUCCESS)
}
