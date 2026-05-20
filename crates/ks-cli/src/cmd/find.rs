use crate::output::{print_tree, print_warn};

pub fn run(config: &ks::Config, query: &str) -> ks::Result<()> {
    let vault = super::open_vault(config)?;
    let results = vault.find(query);
    if results.is_empty() {
        print_warn(&format!("No secrets matching '{query}'"));
    } else {
        print_tree(&results);
    }
    Ok(())
}
