use cliclack::{confirm, intro, outro, password};
use ks::{Config, Vault};
use zeroize::Zeroizing;

use crate::output::{print_info, print_success};

pub fn run(config: &Config) -> ks::Result<()> {
    intro("ks — key store")?;

    let pass: String = password("Enter master passphrase").mask('•').interact()?;
    let confirm_pass: String = password("Confirm passphrase").mask('•').interact()?;

    if pass != confirm_pass {
        cliclack::outro_cancel("Passphrases do not match")?;
        return Ok(());
    }

    if pass.len() < 8 {
        let ok = confirm("Passphrase is shorter than 8 characters — continue anyway?")
            .initial_value(false)
            .interact()?;
        if !ok {
            cliclack::outro_cancel("Aborted")?;
            return Ok(());
        }
    }

    let passphrase = Zeroizing::new(pass);
    let vault = Vault::create(config, passphrase.clone())?;

    ks::session::set_passphrase(&passphrase)?;

    print_success(&format!(
        "Vault created at {}",
        vault.vault_path().display()
    ));
    print_info("Session cached — subsequent commands need no passphrase");
    outro("Run `ks set <path>` to store your first secret.")?;
    Ok(())
}
