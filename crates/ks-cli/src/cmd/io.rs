use std::io::Read as _;

use crate::output::{print_info, print_success};

pub fn run_export(config: &ks::Config, output: Option<&str>) -> ks::Result<()> {
    let vault = super::open_vault(config)?;
    let json = vault.export_json()?;

    if let Some(path) = output {
        std::fs::write(path, &json).map_err(ks::Error::Io)?;
        print_success(&format!("Exported to {path}"));
    } else {
        println!("{json}");
    }
    Ok(())
}

pub fn run_import(config: &ks::Config, file: Option<&str>, dotenv: bool) -> ks::Result<()> {
    let content = if let Some(path) = file { std::fs::read_to_string(path).map_err(ks::Error::Io)? } else {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .map_err(ks::Error::Io)?;
        buf
    };

    let mut vault = super::open_vault(config)?;

    let count = if dotenv {
        vault.import_dotenv(&content)
    } else {
        vault.import_json(&content)?
    };

    vault.save()?;
    print_success(&format!("Imported {count} secret(s)"));
    print_info("Run `ks ls` to verify.");
    Ok(())
}
