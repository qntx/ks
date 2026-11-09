//! Git plumbing for the vault: auto-commit and `ks sync`.
//!
//! Knows paths and commit messages only — it never sees secrets and never
//! decrypts anything. All calls go through the system `git` binary.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

pub use crate::error::ConflictKind;
use crate::{Error, Result};

/// Conflict-resolution direction for `git pull --rebase -X`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keep {
    /// Keep local changes (`-X theirs` during rebase).
    Local,
    /// Keep remote changes (`-X ours` during rebase).
    Remote,
}

/// What a `sync` run did.
#[derive(Debug, Default)]
pub struct SyncReport {
    /// Uncommitted changes were committed before pulling.
    pub committed: bool,
    /// A pull ran.
    pub pulled: bool,
    /// A push ran.
    pub pushed: bool,
    /// Non-fatal observations (e.g. the remote deleted vault metadata).
    pub warnings: Vec<String>,
}

/// One-shot view of the sync-relevant repo state, from a single
/// `git status --porcelain=v2 --branch` scoped to [`STAGE_PATHS`].
/// `ahead`/`behind` reflect the last fetch — `status` never goes online.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct GitStatus {
    /// Current branch name; `None` on a detached HEAD.
    pub branch: Option<String>,
    /// Configured upstream; `None` when the branch has none.
    pub upstream: Option<String>,
    /// Commits ahead of the upstream.
    pub ahead: u32,
    /// Commits behind the upstream.
    pub behind: u32,
    /// Uncommitted entry/metadata changes inside [`STAGE_PATHS`].
    pub changes: u32,
}

/// Paths `ks sync` is allowed to stage: every entry file plus the two
/// canonical git metadata files. A tampered `.gitignore` cannot widen
/// this set, and `-f` in `add` defeats a `.gitignore` that was tampered
/// to *ignore* entries. Also the scope of "dirty" checks.
const STAGE_PATHS: &[&str] = &["--", "*.age", ".gitignore", ".gitattributes"];

/// Thin wrapper over `git -C <root>` for one vault directory.
#[derive(Debug)]
pub struct Git {
    root: PathBuf,
    allow_prompt: bool,
}

impl Git {
    /// Binds to `root`; `allow_prompt` decides whether git may ask for
    /// credentials on the terminal (`GIT_TERMINAL_PROMPT=0` otherwise).
    #[must_use]
    pub fn new(root: &Path, allow_prompt: bool) -> Self {
        Self {
            root: root.to_path_buf(),
            allow_prompt,
        }
    }

    /// git checkouts create files 0644 and dirs 0755 — the vault
    /// requires 600/700, so cloned/pulled trees are re-permissioned
    /// (skipping `.git`, which git manages itself). Otherwise every
    /// clone and every pull that touches files trips doctor.
    #[cfg(unix)]
    fn tighten_perms(&self) {
        let mut stack = vec![self.root.clone()];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                tighten_one(&mut stack, entry.path().as_path());
            }
        }
    }

    #[cfg(not(unix))]
    #[allow(
        clippy::unused_self,
        reason = "same signature as the unix implementation"
    )]
    const fn tighten_perms(&self) {}

    /// True when `<root>/.git` exists (a directory, or a file for worktrees).
    #[must_use]
    pub fn is_repo(&self) -> bool {
        self.root.join(".git").exists()
    }

    /// `git status --porcelain` is empty (or `root` is not a repo).
    ///
    /// # Errors
    ///
    /// [`Error::Unavailable`] when git fails.
    pub fn is_clean(&self) -> Result<bool> {
        if !self.is_repo() {
            return Ok(true);
        }
        let out = self.git(&["status", "--porcelain"])?;
        Ok(out.stdout.is_empty())
    }

    /// Branch, upstream, ahead/behind and uncommitted-change counts in
    /// one call. Defensive: unrecognized lines are ignored, never fatal.
    ///
    /// # Errors
    ///
    /// [`Error::Unavailable`] when `root` is not a repo or git fails.
    pub fn status(&self) -> Result<GitStatus> {
        // `--no-optional-locks`: a status read never takes `index.lock`, so
        // it cannot contend with a concurrent commit or sync.
        let out = self.git(
            &[
                &[
                    "--no-optional-locks",
                    "status",
                    "--porcelain=v2",
                    "--branch",
                ],
                STAGE_PATHS,
            ]
            .concat(),
        )?;
        let text = String::from_utf8_lossy(&out.stdout);
        let mut status = GitStatus::default();
        for line in text.lines() {
            if let Some(head) = line.strip_prefix("# branch.head ") {
                status.branch = (head != "(detached)").then(|| head.to_owned());
            } else if let Some(upstream) = line.strip_prefix("# branch.upstream ") {
                status.upstream = Some(upstream.to_owned());
            } else if let Some(ab) = line.strip_prefix("# branch.ab ") {
                (status.ahead, status.behind) = parse_ahead_behind(ab);
            } else if line.starts_with("1 ")
                || line.starts_with("2 ")
                || line.starts_with("u ")
                || line.starts_with("? ")
            {
                status.changes += 1;
            }
        }
        Ok(status)
    }

    /// `git init` + `git add -A` + one initial commit. The repo always
    /// exists when this returns; a failed initial commit (e.g. no git
    /// identity configured) becomes a returned warning, not an error.
    ///
    /// # Errors
    ///
    /// [`Error::Unavailable`] when git is missing or `git init`/`add`
    /// fails.
    pub fn init(&self, message: &str) -> Result<Vec<String>> {
        self.git(&["init"])?;
        self.git(&["add", "-A"])?;
        match self.git(&["commit", "-q", "-m", message]) {
            Ok(_) => Ok(Vec::new()),
            Err(e) if self.is_repo() => Ok(vec![format!("initial commit skipped: {e}")]),
            Err(e) => Err(e),
        }
    }

    /// `git clone <url> <root>`; `root` must not exist or be empty.
    ///
    /// # Errors
    ///
    /// [`Error::Unavailable`] when git is missing or the clone fails.
    pub fn clone(url: &str, root: &Path, allow_prompt: bool) -> Result<Self> {
        let mut cmd = Command::new("git");
        cmd.args(["clone", "--", url]).arg(root);
        if !allow_prompt {
            cmd.env("GIT_TERMINAL_PROMPT", "0");
        }
        let out = cmd.output().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Error::Unavailable("git not found".to_owned())
            } else {
                Error::Io {
                    path: root.to_path_buf(),
                    source: e,
                }
            }
        })?;
        if !out.status.success() {
            return Err(Error::Unavailable(format!(
                "git clone failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }
        let g = Self::new(root, allow_prompt);
        g.tighten_perms();
        Ok(g)
    }

    /// Commits exactly `paths` (relative or absolute under the root) with
    /// `message`, ignoring other staged or unstaged changes. Only paths that
    /// exist on disk or are tracked are committed; nothing to do -> `false`.
    ///
    /// # Errors
    ///
    /// [`Error::Unavailable`] when git is missing or a step fails.
    pub fn commit_paths(&self, paths: &[PathBuf], message: &str) -> Result<bool> {
        if !self.is_repo() {
            return Ok(false);
        }
        let kept: Vec<String> = paths.iter().filter_map(|p| self.committable(p)).collect();
        if kept.is_empty() {
            return Ok(false);
        }
        self.git_args("add", &["-A", "--"], &kept)?;
        let quiet = self.run(&["diff", "--cached", "--quiet", "--"], &kept)?;
        if quiet.status.success() {
            return Ok(false);
        }
        self.git_args("commit", &["-q", "-m", message, "--"], &kept)?;
        Ok(true)
    }

    /// A path under the root worth committing (exists or tracked).
    fn committable(&self, path: &Path) -> Option<String> {
        let rel = path.strip_prefix(&self.root).unwrap_or(path);
        let rel = rel.to_string_lossy().into_owned();
        if self.root.join(rel.as_str()).exists() {
            return Some(rel);
        }
        let out = self
            .run(&["ls-files", "--error-unmatch", "--", rel.as_str()], &[])
            .ok()?;
        out.status.success().then_some(rel)
    }

    /// commit -> pull --rebase (optionally `-X`) -> push -> sanity check.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidInput`] when the vault is not a repo, has a detached
    /// HEAD or an ambiguous remote setup; [`Error::Conflict`] on rebase
    /// conflicts (the rebase is already aborted); [`Error::Unavailable`]
    /// when a git command fails.
    pub fn sync(&self, keep: Option<Keep>) -> Result<SyncReport> {
        if !self.is_repo() {
            return Err(Error::InvalidInput(
                "vault is not a git repository; use `ks init --git` or `ks git init`".into(),
            ));
        }
        // Detached HEAD cannot be synced.
        if !self
            .run(&["symbolic-ref", "-q", "HEAD"], &[])?
            .status
            .success()
        {
            return Err(Error::InvalidInput("detached HEAD; cannot sync".into()));
        }
        let mut report = SyncReport::default();

        // 1. Commit pending changes — staged by pathspec, not by the
        // (remote-modifiable) .gitignore whitelist.
        let status = self.git(&[&["status", "--porcelain"], STAGE_PATHS].concat())?;
        if !String::from_utf8_lossy(&status.stdout).trim().is_empty() {
            self.git(&[&["add", "-A", "-f"], STAGE_PATHS].concat())?;
            self.git(&["commit", "-q", "-m", "ks: sync"])?;
            report.committed = true;
        }

        // 2. Pull --rebase.
        let strategy: &[&str] = match keep {
            Some(Keep::Local) => &["-X", "theirs"],
            Some(Keep::Remote) => &["-X", "ours"],
            None => &[],
        };
        let upstream = self
            .run(
                &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"],
                &[],
            )?
            .status
            .success();
        let mut remote_push: Option<String> = None;
        if upstream {
            let args = concat_args(&["pull", "--rebase"], strategy, &[]);
            self.pull(&args)?;
            report.pulled = true;
        } else {
            remote_push = self.resolve_remote_pull(strategy, &mut report)?;
        }

        // 3. Push.
        match &remote_push {
            None => self.git(&["push"])?,
            Some(remote) => self.git(&["push", "-u", remote.as_str(), "HEAD"])?,
        };
        report.pushed = true;

        // 4. The remote must not have eaten the vault metadata.
        for file in [".identity.age", ".check.age"] {
            if !self.root.join(file).exists() {
                report.warnings.push(format!(
                    "the remote deleted {file}; restore it from git history"
                ));
            }
        }
        self.tighten_perms();
        Ok(report)
    }

    /// Resolves the no-upstream case: zero remotes or several -> `InvalidInput`;
    /// exactly one remote pulls its same-named branch when it exists and
    /// returns the remote name for `push -u`.
    fn resolve_remote_pull(
        &self,
        strategy: &[&str],
        report: &mut SyncReport,
    ) -> Result<Option<String>> {
        let out = self.git(&["remote"])?;
        let remotes_text = String::from_utf8_lossy(&out.stdout).into_owned();
        let remotes: Vec<&str> = remotes_text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect();
        match remotes.as_slice() {
            [] => Err(Error::InvalidInput(
                "no remote configured; add one with `ks git remote add origin <url>`".into(),
            )),
            [remote] => {
                let remote = (*remote).to_owned();
                let head = self.git(&["symbolic-ref", "--short", "HEAD"])?;
                let branch = String::from_utf8_lossy(&head.stdout).trim().to_owned();
                let probe = self.run(
                    &[
                        "ls-remote",
                        "--exit-code",
                        "--heads",
                        remote.as_str(),
                        branch.as_str(),
                    ],
                    &[],
                )?;
                match probe.status.code() {
                    Some(0) => {
                        let mut args = concat_args(&["pull", "--rebase"], strategy, &[]);
                        args.push(remote.clone());
                        args.push(branch);
                        self.pull(&args)?;
                        report.pulled = true;
                    }
                    Some(2) => {}
                    _ => {
                        return Err(Error::Unavailable(format!(
                            "git ls-remote failed: {}",
                            String::from_utf8_lossy(&probe.stderr).trim()
                        )));
                    }
                }
                Ok(Some(remote))
            }
            _ => Err(Error::InvalidInput(
                "no upstream and multiple remotes; set one with \
                 `ks git branch --set-upstream-to` or `ks git push -u`"
                    .into(),
            )),
        }
    }

    /// Runs `git pull`; on failure distinguishes "rebase in progress" (-> abort
    /// + [`Error::Conflict`]) from plain failure (-> [`Error::Unavailable`]).
    fn pull(&self, args: &[String]) -> Result<()> {
        let out = self.run_strs(args, &[])?;
        if out.status.success() {
            return Ok(());
        }
        if self.rebase_in_progress()? {
            let status = self.run(&["status", "--porcelain=v1", "-z"], &[])?;
            let (entries, kind) = unmerged_paths(&status.stdout);
            self.git(&["rebase", "--abort"])?;
            return Err(Error::Conflict { entries, kind });
        }
        Err(Error::Unavailable(format!(
            "git pull failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )))
    }

    /// True while a rebase is in progress under the real git dir.
    fn rebase_in_progress(&self) -> Result<bool> {
        let git_dir = self.git(&["rev-parse", "--git-dir"])?;
        let git_dir = String::from_utf8_lossy(&git_dir.stdout).trim().to_owned();
        let dir = PathBuf::from(&git_dir);
        let dir = if dir.is_absolute() {
            dir
        } else {
            self.root.join(dir)
        };
        Ok(dir.join("rebase-merge").exists() || dir.join("rebase-apply").exists())
    }

    /// `git -C root <sub> <fixed> <rest>`; non-zero exit -> Unavailable.
    fn git_args(&self, sub: &str, fixed: &[&str], rest: &[String]) -> Result<Output> {
        let mut all: Vec<&str> = Vec::with_capacity(fixed.len() + 1);
        all.push(sub);
        all.extend_from_slice(fixed);
        let out = self.run(&all, rest)?;
        if out.status.success() {
            Ok(out)
        } else {
            Err(Error::Unavailable(format!(
                "git {sub} failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            )))
        }
    }

    /// `git -C root <args>`; non-zero exit -> Unavailable.
    fn git(&self, args: &[&str]) -> Result<Output> {
        let out = self.run(args, &[])?;
        if out.status.success() {
            Ok(out)
        } else {
            Err(Error::Unavailable(format!(
                "git {} failed: {}",
                args.first().copied().unwrap_or("command"),
                String::from_utf8_lossy(&out.stderr).trim()
            )))
        }
    }

    /// Runs git capturing stdout/stderr.
    fn run(&self, args: &[&str], rest: &[String]) -> Result<Output> {
        let mut cmd = Command::new("git");
        cmd.arg("-C").arg(&self.root).args(args).args(rest);
        if !self.allow_prompt {
            cmd.env("GIT_TERMINAL_PROMPT", "0");
        }
        cmd.output().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Error::Unavailable("git not found".to_owned())
            } else {
                Error::Io {
                    path: self.root.clone(),
                    source: e,
                }
            }
        })
    }

    fn run_strs(&self, args: &[String], rest: &[String]) -> Result<Output> {
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        self.run(&refs, rest)
    }
}

#[cfg(unix)]
fn tighten_one(stack: &mut Vec<PathBuf>, path: &Path) {
    use std::os::unix::fs::PermissionsExt as _;
    if path.file_name().is_some_and(|n| n == ".git") {
        return;
    }
    let Ok(meta) = path.symlink_metadata() else {
        return;
    };
    // `set_permissions` follows symlinks: a remote-committed link must never
    // let ks chmod a file outside the vault. Doctor reports the link itself.
    if meta.file_type().is_symlink() {
        return;
    }
    let mode = if meta.is_dir() { 0o700 } else { 0o600 };
    drop(std::fs::set_permissions(
        path,
        std::fs::Permissions::from_mode(mode),
    ));
    if meta.is_dir() {
        stack.push(path.to_path_buf());
    }
}

fn concat_args(a: &[&str], b: &[&str], c: &[String]) -> Vec<String> {
    a.iter()
        .chain(b.iter())
        .map(|s| (*s).to_owned())
        .chain(c.iter().cloned())
        .collect()
}

/// Parses `git status --porcelain=v1 -z` for unmerged entries:
/// `(entry names with .age stripped, kind)`. Unmerged XY codes are DD AU UD
/// UA DU AA UU; any `D` in XY marks a delete conflict.
/// Parses a `branch.ab` field (`+A -B`) into `(ahead, behind)`;
/// malformed input stays 0/0.
fn parse_ahead_behind(ab: &str) -> (u32, u32) {
    let mut it = ab.split_whitespace();
    let (Some(a), Some(b)) = (it.next(), it.next()) else {
        return (0, 0);
    };
    (
        a.trim_start_matches('+').parse().unwrap_or(0),
        b.trim_start_matches('-').parse().unwrap_or(0),
    )
}

pub(crate) fn unmerged_paths(porcelain: &[u8]) -> (Vec<String>, ConflictKind) {
    let mut entries = Vec::new();
    let mut kind = ConflictKind::Content;
    for record in porcelain.split(|b| *b == 0).filter(|r| !r.is_empty()) {
        if record.len() < 3 {
            continue;
        }
        let Some(xy) = record.get(..2) else {
            continue;
        };
        let unmerged = matches!(xy, b"DD" | b"AU" | b"UD" | b"UA" | b"DU" | b"AA" | b"UU");
        if !unmerged {
            continue;
        }
        if xy.contains(&b'D') {
            kind = ConflictKind::Delete;
        }
        let Some(rest) = record.get(3..) else {
            continue;
        };
        let path = String::from_utf8_lossy(rest).into_owned();
        entries.push(path.strip_suffix(".age").map(str::to_owned).unwrap_or(path));
    }
    entries.sort();
    (entries, kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    #[allow(
        clippy::unwrap_used,
        reason = "test fixture setup may panic on filesystem errors"
    )]
    fn tighten_perms_never_follows_symlinks() {
        use std::os::unix::fs::PermissionsExt as _;
        let outside = tempfile::tempdir().unwrap();
        let target = outside.path().join("tool");
        std::fs::write(&target, b"#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o755)).unwrap();
        let vault = tempfile::tempdir().unwrap();
        std::fs::write(vault.path().join("x.age"), b"").unwrap();
        std::os::unix::fs::symlink(&target, vault.path().join("link.age")).unwrap();

        Git::new(vault.path(), false).tighten_perms();

        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(
            mode(&target),
            0o755,
            "symlink target outside the vault was changed"
        );
        assert_eq!(
            mode(&vault.path().join("x.age")),
            0o600,
            "entry was not tightened"
        );
    }

    #[test]
    fn unmerged_parsing() {
        let (entries, kind) = unmerged_paths(b"UU a.age\0DU dir/x.age\0 M clean.age\0AU y\0");
        assert_eq!(entries, ["a", "dir/x", "y"]);
        assert_eq!(kind, ConflictKind::Delete);

        let (entries2, kind2) = unmerged_paths(b"AA only.age\0");
        assert_eq!(entries2, ["only"]);
        assert_eq!(kind2, ConflictKind::Content);

        // rename record: "XY orig\0new\0" - the second path is ignored as a record
        let (entries3, kind3) = unmerged_paths(b"R  old.age\0new.age\0UU both.age\0");
        assert_eq!(entries3, ["both"]);
        assert_eq!(kind3, ConflictKind::Content);
    }
    /// Runs `git -C dir <args>` and asserts success.
    fn git(dir: &Path, args: &[&str]) {
        let st = Command::new("git")
            .args(["-C"])
            .arg(dir)
            .args(args)
            .status()
            .unwrap();
        assert!(st.success(), "git {args:?} failed");
    }

    /// A repo with a repo-local identity, one commit, a bare remote and
    /// `origin` wired + fetched upstream.
    fn repo_with_remote(root: &Path, remote: &Path) -> Git {
        git(root, &["init", "-q"]);
        git(root, &["config", "user.name", "ks-test"]);
        git(root, &["config", "user.email", "t@t"]);
        git(root, &["config", "commit.gpgsign", "false"]);
        std::fs::write(root.join("a.age"), b"x").unwrap();
        git(root, &["add", "-Af", "--", "*.age"]);
        git(root, &["commit", "-qm", "init"]);
        git(remote, &["init", "-q", "--bare"]);
        git(root, &["remote", "add", "origin", remote.to_str().unwrap()]);
        git(root, &["push", "-q", "-u", "origin", "HEAD"]);
        git(root, &["fetch", "-q"]);
        Git::new(root, false)
    }

    /// `status()`: branch, upstream, ahead/behind and staged-path changes.
    #[test]
    fn git_status_reports() {
        let dir = tempfile::tempdir().unwrap();
        let remote = tempfile::tempdir().unwrap();
        let root = dir.path().join("v");
        std::fs::create_dir(&root).unwrap();
        let bare = remote.path().join("r");
        std::fs::create_dir(&bare).unwrap();
        let g = repo_with_remote(&root, &bare);

        // clean with an upstream
        let st = g.status().unwrap();
        assert_eq!(st.branch.as_deref(), Some("master"));
        assert_eq!(st.upstream.as_deref(), Some("origin/master"));
        assert_eq!((st.ahead, st.behind, st.changes), (0, 0, 0));

        // a modified .age counts; a non-.age file does not
        std::fs::write(root.join("a.age"), b"y").unwrap();
        std::fs::write(root.join("notes.txt"), b"n").unwrap();
        std::fs::write(root.join("new.age"), b"z").unwrap();
        let dirty = g.status().unwrap();
        assert_eq!(dirty.changes, 2);

        // ahead after a local commit
        git(&root, &["add", "-Af", "--", "*.age"]);
        git(&root, &["commit", "-qm", "c2"]);
        assert_eq!(g.status().unwrap().ahead, 1);

        // behind after another clone pushes and this one fetches
        let other = dir.path().join("o");
        git(dir.path(), &["clone", "-q", bare.to_str().unwrap(), "o"]);
        git(&other, &["config", "user.name", "ks-test"]);
        git(&other, &["config", "user.email", "t@t"]);
        git(&other, &["config", "commit.gpgsign", "false"]);
        std::fs::write(other.join("b.age"), b"b").unwrap();
        git(&other, &["add", "-Af", "--", "*.age"]);
        git(&other, &["commit", "-qm", "remote"]);
        git(&other, &["push", "-q"]);
        git(&root, &["fetch", "-q"]);
        let after = g.status().unwrap();
        assert_eq!((after.ahead, after.behind), (1, 1));
    }

    /// `status()` on a repo without an upstream reports None.
    #[test]
    fn git_status_no_upstream() {
        let root = tempfile::tempdir().unwrap();
        git(root.path(), &["init", "-q"]);
        git(root.path(), &["config", "user.name", "ks-test"]);
        git(root.path(), &["config", "user.email", "t@t"]);
        git(root.path(), &["config", "commit.gpgsign", "false"]);
        std::fs::write(root.path().join("a.age"), b"x").unwrap();
        git(root.path(), &["add", "-Af", "--", "*.age"]);
        git(root.path(), &["commit", "-qm", "init"]);
        let st = Git::new(root.path(), false).status().unwrap();
        assert!(st.branch.is_some());
        assert_eq!(st.upstream, None);
    }
}
