//! `ks ls [<prefix>]` — list entries, no unlock.

use super::{Cli, CliResult, Out, err, name, vault};

/// Lists names sorted; human mode renders an indented tree.
pub(crate) fn run(cli: &Cli, out: &Out, prefix: Option<&str>) -> CliResult<i32> {
    let v = vault(cli)?;
    let prefix = prefix.map(name).transpose()?;
    let names = v.list(prefix.as_ref()).map_err(|e| err(&e))?;
    let json: Vec<String> = names.iter().map(ToString::to_string).collect();
    let mut tree = String::new();
    let mut prev: Vec<&str> = Vec::new();
    for n in &json {
        let segs: Vec<&str> = n.split('/').collect();
        let common = segs.iter().zip(&prev).take_while(|(a, b)| a == b).count();
        for (i, seg) in segs.iter().enumerate().skip(common) {
            tree.push_str(&"  ".repeat(i));
            tree.push_str(seg);
            tree.push('\n');
        }
        prev = segs;
    }
    Ok(out.ok_value(tree.trim_end(), serde_json::json!({ "entries": json })))
}
