//! `ks doctor [--fix]` — check the vault for problems.

use std::path::{Path, PathBuf};

use super::{Cli, CliResult, Out, err, unlock};

#[derive(Debug)]
struct Problem {
    path: PathBuf,
    problem: String,
    fixable: bool,
}

/// Reports structural problems; `--fix` removes `.tmp*` files and repairs
/// permissions.
pub(crate) fn run(cli: &Cli, out: &Out, fix: bool) -> CliResult<i32> {
    let session = unlock(cli, out)?;
    let root = session.vault().root().to_path_buf();
    let mut problems = Vec::new();
    scan(&root, &mut problems);

    // Every listed entry must decrypt with a matching name.
    let names = session.vault().list(None).map_err(|e| err(&e))?;
    for n in &names {
        if let Err(e) = session.get(n) {
            let hint = match e {
                ks::Error::Corrupt { .. } => {
                    format!("{e} (possibly written before a key rotation)")
                }
                _ => e.to_string(),
            };
            problems.push(Problem {
                path: n.path_in(&root),
                problem: hint,
                fixable: false,
            });
        }
    }

    if fix {
        problems.retain(|p| !p.fixable || !apply_fix(p));
    }
    let ok = problems.is_empty();
    let json_problems: Vec<serde_json::Value> = problems
        .iter()
        .map(|p| {
            serde_json::json!({
                "path": p.path.display().to_string(),
                "problem": p.problem,
            })
        })
        .collect();
    let human = if ok {
        "ok".to_owned()
    } else {
        problems
            .iter()
            .map(|p| format!("{}: {}", p.path.display(), p.problem))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let code = out.ok_value(
        &human,
        serde_json::json!({ "ok": ok, "problems": json_problems }),
    );
    if ok { Ok(code) } else { Ok(65) }
}

#[cfg(unix)]
fn fix_perms(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    let mode = if path.is_dir() { 0o700 } else { 0o600 };
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).is_ok()
}

#[cfg(not(unix))]
const fn fix_perms(_path: &Path) -> bool {
    false
}

/// Applies a `--fix` for one problem; returns true when resolved.
fn apply_fix(p: &Problem) -> bool {
    match p.problem.as_str() {
        s if s.starts_with("leftover temp file") => std::fs::remove_file(&p.path).is_ok(),
        s if s.starts_with("wrong permissions") => fix_perms(&p.path),
        _ => false,
    }
}

/// Files the vault manages besides `*.age` entries.
const INTERNAL: &[&str] = &[
    ".identity.age",
    ".check.age",
    ".lock",
    ".gitattributes",
    ".gitignore",
];

fn scan(root: &Path, problems: &mut Vec<Problem>) {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(meta) = path.symlink_metadata() else {
                continue;
            };
            let name = entry.file_name();
            let name = name.to_str().unwrap_or("");
            if meta.file_type().is_symlink() {
                problems.push(Problem {
                    path: path.clone(),
                    problem: "symlinks are not allowed inside the vault".into(),
                    fixable: false,
                });
                continue;
            }
            if meta.is_dir() {
                stack.push(path.clone());
                check_perms(&path, &meta.permissions(), 0o700, problems);
                continue;
            }
            if name.starts_with(".tmp") {
                problems.push(Problem {
                    path: path.clone(),
                    problem: "leftover temp file".into(),
                    fixable: true,
                });
                continue;
            }
            let is_internal = INTERNAL.contains(&name);
            let is_entry = name.rsplit('.').next() == Some("age") && !name.starts_with('.');
            if !is_internal && !is_entry {
                problems.push(Problem {
                    path: path.clone(),
                    problem: "non-ks file inside the vault".into(),
                    fixable: false,
                });
                continue;
            }
            if name == ".lock" {
                continue; // lock file perms don't matter
            }
            check_perms(&path, &meta.permissions(), 0o600, problems);
        }
    }
}

#[cfg(unix)]
fn check_perms(
    path: &Path,
    perms: &std::fs::Permissions,
    expected: u32,
    problems: &mut Vec<Problem>,
) {
    use std::os::unix::fs::PermissionsExt as _;
    if perms.mode() & 0o777 != expected {
        problems.push(Problem {
            path: path.to_path_buf(),
            problem: format!("wrong permissions (expected {expected:o})"),
            fixable: true,
        });
    }
}

#[cfg(not(unix))]
const fn check_perms(_p: &Path, _perms: &std::fs::Permissions, _e: u32, _problems: &mut Vec<Problem>) {}
