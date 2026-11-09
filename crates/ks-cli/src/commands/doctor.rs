//! `ks doctor [--fix]` — check the vault for problems.

use super::{Cli, CliResult, Out, err, unlock};

/// Reports structural problems; `--fix` removes `.tmp*` files and repairs
/// permissions and git metadata.
pub(crate) fn run(cli: &Cli, out: &Out, fix: bool) -> CliResult<i32> {
    let session = unlock(cli, out)?;
    let report = ks::doctor::check(&session).map_err(|e| err(&e))?;
    let problems = if fix {
        ks::doctor::fix(report).problems
    } else {
        report.problems
    };

    let ok = problems.is_empty();
    let json_problems: Vec<serde_json::Value> = problems
        .iter()
        .map(|p| {
            serde_json::json!({
                "path": p.path.display().to_string(),
                "problem": p.message,
            })
        })
        .collect();
    let human = if ok {
        "ok".to_owned()
    } else {
        problems
            .iter()
            .map(|p| format!("{}: {}", p.path.display(), p.message))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let code = out.ok_value(
        &human,
        serde_json::json!({ "ok": ok, "problems": json_problems }),
    );
    if ok { Ok(code) } else { Ok(65) }
}
