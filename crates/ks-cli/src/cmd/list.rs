use crate::output::print_tree;

pub fn run(config: &ks::Config, prefix: &str) -> ks::Result<()> {
    let vault = super::open_vault(config)?;
    let paths = vault.list(prefix);
    print_tree(&paths);
    Ok(())
}
