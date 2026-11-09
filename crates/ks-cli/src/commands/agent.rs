//! `ks __agent` — the session agent daemon (hidden, spawned by `ks unlock`).
//! The lifecycle lives in `ks::agent::Server`; this only reads the identity
//! from stdin, hardens, prints `ready` and waits.

use std::io::{Read as _, Write as _};

use age::secrecy::SecretString;
use ks::ErrorCode;
use zeroize::Zeroizing;

use super::{Cli, CliResult, Out, err, vault};

/// Unix: reads the vault key from stdin (to EOF), applies hardening, binds
/// the endpoint, prints `ready`, then serves until idle/lifetime expiry or
/// a `lock` request.
pub(crate) fn run(cli: &Cli, _out: &Out, ttl: u64, max: u64) -> CliResult<i32> {
    let v = vault(cli)?;

    let mut text = Zeroizing::new(String::new());
    std::io::stdin()
        .read_to_string(&mut text)
        .map_err(|e| (ErrorCode::Io, format!("cannot read agent identity: {e}")))?;
    crate::harden::agent();
    let server = ks::agent::Server::start(
        &v,
        SecretString::from(std::mem::take(&mut *text)),
        ks::agent::ServerConfig {
            ttl: std::time::Duration::from_secs(ttl),
            max: std::time::Duration::from_secs(max),
            host: ks::agent::Host::Cli,
        },
    )
    .map_err(|e| err(&e))?;

    // Signal ks unlock, then stop talking to the terminal.
    drop(writeln!(std::io::stdout(), "ready"));
    drop(std::io::stdout().flush());

    let _reason = server.wait();
    Ok(0)
}
