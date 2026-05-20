use crate::output::print_success;

pub fn run() -> ks::Result<()> {
    ks::session::clear()?;
    print_success("Vault locked (session cleared)");
    Ok(())
}
