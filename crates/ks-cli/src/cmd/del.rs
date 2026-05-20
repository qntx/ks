use cliclack::confirm;

use crate::output::{print_success, print_warn};

pub fn run(config: &ks::Config, path: &str, force: bool) -> ks::Result<()> {
    let mut vault = super::open_vault(config)?;

    if !force {
        let ok = confirm(format!("Delete {path}?"))
            .initial_value(false)
            .interact()
            .map_err(|e| ks::Error::Io(std::io::Error::other(e.to_string())))?;
        if !ok {
            print_warn("Aborted");
            return Ok(());
        }
    }

    vault.delete(path)?;
    vault.save()?;
    print_success(&format!("Deleted {path}"));
    Ok(())
}
