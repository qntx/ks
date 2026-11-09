//! Vault health check (`ks doctor`, desktop "Check vault"): scans the
//! vault for structural problems and applies safe repairs.
//!
//! The library never prints; callers format [`Problem::message`], which
//! is byte-identical to what `ks doctor` prints.

use std::path::{Path, PathBuf};

use crate::Result;
use crate::vault::Session;

/// The structural problem class; fixes and UI groupings key off this.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProblemKind {
    /// A second identity file — `ks rekey` was interrupted mid-rotation.
    UnfinishedRotation,
    /// An entry that does not decrypt with its matching name.
    Undecryptable,
    /// A `.tmp*` file left behind by a crashed write.
    TempFile,
    /// Vault files/dirs not at 600/700.
    Permissions,
    /// `.gitignore`/`.gitattributes` differ from the canonical content.
    GitMeta,
    /// A required file could not be read at all.
    Unreadable,
    /// A symlink inside the vault.
    Symlink,
    /// A file ks does not manage inside the vault.
    NonKsFile,
}

/// One finding: the file involved, its class and the user-facing text.
#[derive(Debug)]
pub struct Problem {
    /// The offending path.
    pub path: PathBuf,
    /// What kind of problem this is.
    pub kind: ProblemKind,
    /// Byte-identical to the `ks doctor` problem line.
    pub message: String,
}

impl Problem {
    /// True when [`fix`] knows how to repair this problem.
    #[must_use]
    pub const fn fixable(&self) -> bool {
        matches!(
            self.kind,
            ProblemKind::TempFile | ProblemKind::Permissions | ProblemKind::GitMeta
        )
    }
}

/// The outcome of a vault scan.
#[derive(Debug)]
pub struct Report {
    /// Every problem found, in scan order.
    pub problems: Vec<Problem>,
}

/// Files the vault manages besides `*.age` entries.
const INTERNAL: &[&str] = &[
    ".identity.age",
    ".check.age",
    ".lock",
    ".gitattributes",
    ".gitignore",
];

/// Scans the vault: unfinished rotation, temp/non-ks/symlink files,
/// permissions, canonical git metadata, and per-entry decryptability.
///
/// # Errors
///
/// [`crate::Error`] when the entry list cannot be read.
pub fn check(session: &Session) -> Result<Report> {
    let root = session.vault().root().to_path_buf();
    let mut problems = Vec::new();
    if session.key_count() > 1 {
        problems.push(Problem {
            path: root.join(".identity.age"),
            kind: ProblemKind::UnfinishedRotation,
            message: "unfinished key rotation; run `ks rekey` with the new passphrase to finish"
                .to_owned(),
        });
    }
    scan(&root, &mut problems);
    check_git_meta(&root, &mut problems);

    // Every listed entry must decrypt with a matching name.
    for n in &session.vault().list(None)? {
        if let Err(e) = session.get(n) {
            let hint = match e {
                crate::Error::Corrupt { .. } => {
                    format!("{e} (possibly written before a key rotation)")
                }
                _ => e.to_string(),
            };
            problems.push(Problem {
                path: n.path_in(&root),
                kind: ProblemKind::Undecryptable,
                message: hint,
            });
        }
    }
    Ok(Report { problems })
}

/// Applies the repairs `fixable()` marks and returns what remains.
#[must_use]
pub fn fix(report: Report) -> Report {
    Report {
        problems: report
            .problems
            .into_iter()
            .filter(|p| !p.fixable() || !apply_fix(p))
            .collect(),
    }
}

/// Applies the fix for one problem; returns true when resolved.
fn apply_fix(p: &Problem) -> bool {
    match p.kind {
        ProblemKind::TempFile => std::fs::remove_file(&p.path).is_ok(),
        ProblemKind::Permissions => fix_perms(&p.path),
        ProblemKind::GitMeta => {
            let canonical = match p.path.file_name().and_then(|n| n.to_str()) {
                Some(".gitignore") => crate::GITIGNORE,
                Some(".gitattributes") => crate::GITATTRIBUTES,
                _ => return false,
            };
            std::fs::write(&p.path, canonical).is_ok()
        }
        _ => false,
    }
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

/// `.gitignore`/`.gitattributes` must equal the canonical content the
/// vault writes — a remote can weaken the whitelist and the next
/// `sync` would then stage files ks never wrote.
fn check_git_meta(root: &Path, problems: &mut Vec<Problem>) {
    for (file, canonical) in [
        (".gitignore", crate::GITIGNORE),
        (".gitattributes", crate::GITATTRIBUTES),
    ] {
        let path = root.join(file);
        match std::fs::read(&path) {
            Ok(bytes) if bytes == canonical.as_bytes() => {}
            Ok(_) => problems.push(Problem {
                path: path.clone(),
                kind: ProblemKind::GitMeta,
                message: format!("{file} differs from the canonical content"),
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => problems.push(Problem {
                path: path.clone(),
                kind: ProblemKind::Unreadable,
                message: format!("cannot read {file}: {e}"),
            }),
        }
    }
}

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
            // `.git` — repository internals are out of scope entirely
            // (a `.git` file covers worktrees): no perms, no reports, no fixes.
            if dir == root && name == ".git" {
                continue;
            }
            if meta.file_type().is_symlink() {
                problems.push(Problem {
                    path: path.clone(),
                    kind: ProblemKind::Symlink,
                    message: "symlinks are not allowed inside the vault".into(),
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
                    kind: ProblemKind::TempFile,
                    message: "leftover temp file".into(),
                });
                continue;
            }
            let is_internal = INTERNAL.contains(&name);
            let is_entry = name.rsplit('.').next() == Some("age") && !name.starts_with('.');
            if !is_internal && !is_entry {
                problems.push(Problem {
                    path: path.clone(),
                    kind: ProblemKind::NonKsFile,
                    message: "non-ks file inside the vault".into(),
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
            kind: ProblemKind::Permissions,
            message: format!("wrong permissions (expected {expected:o})"),
        });
    }
}

#[cfg(not(unix))]
const fn check_perms(
    _p: &Path,
    _perms: &std::fs::Permissions,
    _e: u32,
    _problems: &mut Vec<Problem>,
) {
}

#[cfg(test)]
mod tests {
    use super::*;
    use age::secrecy::SecretString;

    fn session() -> (tempfile::TempDir, Session) {
        let dir = tempfile::tempdir().unwrap();
        let s = crate::Vault::create(
            dir.path(),
            &SecretString::from("correct horse battery staple"),
        )
        .unwrap();
        (dir, s)
    }

    #[test]
    fn doctor_clean_vault() {
        let (_d, s) = session();
        assert!(check(&s).unwrap().problems.is_empty());
    }

    #[test]
    fn doctor_reports_and_fixes() {
        let (dir, s) = session();
        let root = dir.path();
        std::fs::write(root.join(".tmp123"), b"").unwrap();
        std::fs::write(root.join("junk.txt"), b"").unwrap();
        std::fs::write(root.join(".gitignore"), b"weakened\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(
                root.join(".identity.age"),
                std::fs::Permissions::from_mode(0o644),
            )
            .unwrap();
        }
        let report = check(&s).unwrap();
        let kinds: Vec<ProblemKind> = report.problems.iter().map(|p| p.kind).collect();
        assert!(kinds.contains(&ProblemKind::TempFile));
        assert!(kinds.contains(&ProblemKind::NonKsFile));
        assert!(kinds.contains(&ProblemKind::GitMeta));
        #[cfg(unix)]
        assert!(kinds.contains(&ProblemKind::Permissions));

        let remaining = fix(report).problems;
        let left: Vec<ProblemKind> = remaining.iter().map(|p| p.kind).collect();
        assert!(!left.contains(&ProblemKind::TempFile));
        assert!(!left.contains(&ProblemKind::GitMeta));
        #[cfg(unix)]
        assert!(!left.contains(&ProblemKind::Permissions));
        assert!(left.contains(&ProblemKind::NonKsFile)); // unfixable stays
        assert!(check(&s).unwrap().problems.iter().all(|p| !p.fixable()));
    }
}
