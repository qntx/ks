use crate::{clip, output};

pub fn run(config: &ks::Config, path: &str, copy: bool) -> ks::Result<()> {
    let vault = super::open_vault(config)?;
    let secret = vault.get(path)?;

    if copy {
        let secs = clip::copy_with_autoclean(&secret.value)?;
        output::print_info(&format!("Copied {path} to clipboard (clears in {secs}s)"));
    } else {
        println!("{}", &*secret.value);
    }
    Ok(())
}
