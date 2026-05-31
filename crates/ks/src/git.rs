//! Thin wrapper over the system `git` binary.
//!
//! We deliberately shell out instead of pulling in a Rust git library
//! (`gix` etc.): users already have SSH agents, signing keys, credential
//! helpers and ssh config configured for the `git` they use elsewhere, and
//! it is futile to reimplement that surface.

use std::path::Path;
use std::process::{Command, Stdio};

use crate::error::{Error, Result};

const BIN: &str = "git";

/// Returns `true` if `dir` contains a `.git` directory or file.
#[must_use]
pub fn is_repo(dir: &Path) -> bool {
    dir.join(".git").exists()
}

/// Initialises a git repository at `dir` and writes a sensible `.gitattributes`
/// that marks `*.age` as binary so git does not try to diff ciphertext.
///
/// # Errors
/// Returns [`Error::Command`] if `git init` fails or [`Error::Io`] on write.
pub fn init(dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    run(dir, &["init", "--initial-branch=main"])?;
    let gitattributes = dir.join(".gitattributes");
    if !gitattributes.exists() {
        std::fs::write(
            &gitattributes,
            "*.age binary -diff -merge\n.age-recipients text\n",
        )?;
    }
    let gitignore = dir.join(".gitignore");
    if !gitignore.exists() {
        // Local-only runtime files that must never be committed: the advisory
        // lock, atomic-write scratch files, and the rotation staging area.
        std::fs::write(&gitignore, ".ks.lock\n.ks-rotate/\n*.tmp\n")?;
    }
    Ok(())
}

/// `git add -A`.
///
/// # Errors
/// Returns [`Error::Command`] on failure.
pub fn add_all(dir: &Path) -> Result<()> {
    run(dir, &["add", "-A"])?;
    Ok(())
}

/// `git commit -m <message>`. Returns `Ok(())` if there was nothing to commit.
///
/// # Errors
/// Returns [`Error::Command`] only for actual failures, not for the
/// "nothing to commit" case.
pub fn commit(dir: &Path, message: &str) -> Result<()> {
    let output = command(dir, &["commit", "-m", message])
        .output()
        .map_err(Error::Io)?;
    if output.status.success() {
        return Ok(());
    }
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    if combined.contains("nothing to commit") || combined.contains("no changes added") {
        return Ok(());
    }
    Err(Error::Command {
        cmd: format!("git commit -m {message:?}"),
        status: output.status.code().unwrap_or(-1),
        stderr: combined,
    })
}

/// `git pull --rebase --autostash`.
///
/// # Errors
/// Returns [`Error::Command`] on failure.
pub fn pull_rebase(dir: &Path) -> Result<()> {
    run(dir, &["pull", "--rebase", "--autostash"])?;
    Ok(())
}

/// `git push`.
///
/// # Errors
/// Returns [`Error::Command`] on failure.
pub fn push(dir: &Path) -> Result<()> {
    run(dir, &["push"])?;
    Ok(())
}

/// `git status -sb`.
///
/// # Errors
/// Returns [`Error::Command`] on failure.
pub fn status(dir: &Path) -> Result<String> {
    run(dir, &["status", "-sb"])
}

/// `git log -n <n> --oneline`.
///
/// # Errors
/// Returns [`Error::Command`] on failure.
pub fn log(dir: &Path, n: usize) -> Result<String> {
    let limit = format!("-n{n}");
    run(dir, &["log", "--oneline", &limit])
}

fn run(dir: &Path, args: &[&str]) -> Result<String> {
    let output = command(dir, args).output().map_err(Error::Io)?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(Error::Command {
            cmd: format!("git {}", args.join(" ")),
            status: output.status.code().unwrap_or(-1),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

fn command(dir: &Path, args: &[&str]) -> Command {
    let mut cmd = Command::new(BIN);
    cmd.current_dir(dir)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd
}
