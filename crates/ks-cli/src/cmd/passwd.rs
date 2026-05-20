use cliclack::{intro, outro, password};
use zeroize::Zeroizing;

use crate::output::print_success;

pub fn run(config: &ks::Config) -> ks::Result<()> {
    intro("ks — change passphrase")?;

    let mut vault = super::open_vault(config)?;

    let new_raw: String = password("New passphrase")
        .mask('•')
        .interact()
        .map_err(|e| ks::Error::Io(std::io::Error::other(e.to_string())))?;

    let confirm_raw: String = password("Confirm new passphrase")
        .mask('•')
        .interact()
        .map_err(|e| ks::Error::Io(std::io::Error::other(e.to_string())))?;

    if new_raw != confirm_raw {
        cliclack::outro_cancel("Passphrases do not match")?;
        return Ok(());
    }

    let new_passphrase = Zeroizing::new(new_raw);
    vault.change_passphrase(new_passphrase.clone());
    vault.save()?;
    ks::session::set_passphrase(&new_passphrase)?;

    print_success("Passphrase updated");
    outro("Session refreshed.")?;
    Ok(())
}
