use crate::output::print_warn;

pub fn run(config: &ks::Config, paths: &[String], shell: &str) -> ks::Result<()> {
    let vault = super::open_vault(config)?;

    let targets: Vec<&str> = if paths.is_empty() {
        vault.list("")
    } else {
        paths.iter().map(String::as_str).collect()
    };

    if targets.is_empty() {
        print_warn("No secrets found");
        return Ok(());
    }

    for path in targets {
        let secret = vault.get(path)?;
        let var = path.replace('/', "_").to_uppercase();
        let val = &*secret.value;
        match shell {
            "fish" => println!("set -x {var} '{val}'"),
            "powershell" | "ps" => println!("$env:{var} = '{val}'"),
            _ => println!("export {var}='{val}'"),
        }
    }
    Ok(())
}
