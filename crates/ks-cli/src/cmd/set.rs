use std::io::IsTerminal as _;

use cliclack::{confirm, intro, outro, password};
use ks::Secret;

use crate::output::{print_success, print_warn};

pub fn run(config: &ks::Config, path: &str, note: Option<&str>, force: bool) -> ks::Result<()> {
    let mut vault = super::open_vault(config)?;

    if vault.exists(path) && !force {
        intro("ks — update secret")?;
        let ok = confirm(format!("{path} already exists — overwrite?"))
            .initial_value(false)
            .interact()
            .map_err(|e| ks::Error::Io(std::io::Error::other(e.to_string())))?;
        if !ok {
            print_warn("Aborted");
            return Ok(());
        }
    }

    let raw: String = if std::io::stdin().is_terminal() {
        intro("ks — set secret")?;
        let v = password(format!("Value for {path}"))
            .mask('•')
            .interact()
            .map_err(|e| ks::Error::Io(std::io::Error::other(e.to_string())))?;
        outro(format!("Stored {path}"))?;
        v
    } else {
        let mut buf = String::new();
        std::io::stdin()
            .read_line(&mut buf)
            .map_err(ks::Error::Io)?;
        buf.trim_end_matches(['\n', '\r']).to_owned()
    };

    let mut secret = Secret::new(raw);
    if let Some(n) = note {
        secret = secret.with_note(n);
    }

    vault.set(path, secret)?;
    vault.save()?;

    print_success(&format!("Stored {path}"));
    Ok(())
}
