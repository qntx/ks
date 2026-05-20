use ks::generate::{Charset, generate};
use ks::Secret;

use crate::output::{print_info, print_success};

pub fn run(
    config: &ks::Config,
    path: Option<&str>,
    length: usize,
    charset_str: &str,
    force: bool,
) -> ks::Result<()> {
    let charset = match charset_str {
        "hex" => Charset::Hex,
        "printable" => Charset::Printable,
        _ => Charset::Alphanumeric,
    };

    let value = generate(length, charset);

    if let Some(p) = path {
        let mut vault = super::open_vault(config)?;
        if vault.exists(p) && !force {
            return Err(ks::Error::SecretExists(p.to_owned()));
        }
        vault.set(p, Secret::new(value.as_str()))?;
        vault.save()?;
        print_success(&format!("Generated and stored {p}"));
    } else {
        print_info(&format!("Generated ({charset_str}, {length} chars):"));
        println!("{}", &*value);
    }
    Ok(())
}
