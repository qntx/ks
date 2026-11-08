//! `ks sync [--keep local|remote]` — commit, pull --rebase, push.

use ks::ErrorCode;
use ks::git::{Git, Keep};

use super::{Cli, CliResult, Out, err, vault};
use crate::cli::KeepSide;

/// Runs the sync pipeline; the vault must exist and be a repo.
pub(crate) fn run(cli: &Cli, out: &Out, keep: Option<KeepSide>) -> CliResult<i32> {
    let keep = keep.map(|side| match side {
        KeepSide::Local => Keep::Local,
        KeepSide::Remote => Keep::Remote,
    });
    let v = vault(cli)?;
    let git = Git::new(v.root(), out.interactive);
    match git.sync(keep) {
        Ok(report) => {
            for w in &report.warnings {
                out.warn(w.clone());
            }
            if !out.json {
                let mut did = Vec::new();
                if report.committed {
                    did.push("committed");
                }
                if report.pulled {
                    did.push("pulled");
                }
                if report.pushed {
                    did.push("pushed");
                }
                out.note(&format!("synced: {}", did.join(", ")));
            }
            Ok(out.ok_value(
                "",
                serde_json::json!({
                    "committed": report.committed,
                    "pulled": report.pulled,
                    "pushed": report.pushed,
                }),
            ))
        }
        Err(ks::Error::Conflict { entries, kind }) => {
            let message = format!("sync conflicts: {}", entries.join(", "));
            if !out.json {
                out.note(&format!("conflict in: {}", entries.join(", ")));
                match kind {
                    ks::ConflictKind::Content => out.note(
                        "resolve by rerunning: `ks sync --keep local` or `ks sync --keep remote`",
                    ),
                    ks::ConflictKind::Delete => out.note(
                        "one side deleted an entry the other modified: run `ks git pull --rebase`, \
                         resolve with `ks git rm <file>` or `ks git add <file>`, \
                         then `ks git rebase --continue`",
                    ),
                }
            }
            Ok(out.error_details(
                ErrorCode::Conflict,
                &message,
                &serde_json::json!({ "entries": entries, "kind": kind.as_str() }),
            ))
        }
        Err(e) => Err(err(&e)),
    }
}
