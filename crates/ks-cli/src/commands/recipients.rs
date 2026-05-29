//! `ks recipients` — manage the recipient (public key) list.

use std::process::ExitCode;
use std::str::FromStr as _;

use ks::crypto;
use ks::x25519;
use ks::{Config, Error, Result};

use crate::cli::RecipientsCmd;
use crate::commands;
use crate::terminal;

pub fn run(config: &Config, cmd: RecipientsCmd) -> Result<ExitCode> {
    let mut store = commands::open_store(config)?;
    match cmd {
        RecipientsCmd::Ls => {
            for r in store.recipients() {
                println!("{r}");
            }
            Ok(ExitCode::SUCCESS)
        }
        RecipientsCmd::Add { pubkey } => {
            let new = x25519::Recipient::from_str(pubkey.trim())
                .map_err(|e| Error::InvalidRecipient(e.to_owned()))?;
            let mut updated = store.recipients().to_vec();
            if crypto::recipients_contain(&updated, &new) {
                terminal::warn("Recipient already present; nothing to do");
                return Ok(ExitCode::SUCCESS);
            }
            updated.push(new);
            let identity = commands::unlock(config)?;
            let n = store.set_recipients(updated, &identity)?;
            terminal::success(&format!("Added recipient and re-encrypted {n} secret(s)"));
            Ok(ExitCode::SUCCESS)
        }
        RecipientsCmd::Rm { pubkey } => {
            let target = x25519::Recipient::from_str(pubkey.trim())
                .map_err(|e| Error::InvalidRecipient(e.to_owned()))?;
            let target_str = target.to_string();
            let mut updated: Vec<x25519::Recipient> = store
                .recipients()
                .iter()
                .filter(|r| r.to_string() != target_str)
                .cloned()
                .collect();
            if updated.len() == store.recipients().len() {
                terminal::warn("Recipient not found; nothing to do");
                return Ok(ExitCode::SUCCESS);
            }
            let identity = commands::unlock(config)?;
            // Keep the user's own key so they don't lock themselves out.
            let own = identity.to_public();
            if !crypto::recipients_contain(&updated, &own) {
                updated.push(own);
            }
            let n = store.set_recipients(updated, &identity)?;
            terminal::success(&format!("Removed recipient and re-encrypted {n} secret(s)"));
            Ok(ExitCode::SUCCESS)
        }
    }
}
