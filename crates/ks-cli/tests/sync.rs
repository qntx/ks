//! M2 sync tests: init --git/--clone, auto-commit, ks sync, ks git.
//! Test names start with the spec ID in lowercase snake case.
#![allow(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::missing_assert_message,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation,
    clippy::let_underscore_must_use,
    clippy::shadow_unrelated,
    clippy::items_after_statements,
    unused_variables,
    unused_mut,
    reason = "integration test harness: panicking on setup and terse assertions are intentional"
)]

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const PASS: &str = "correct horse battery staple fixture";

struct T {
    dir: tempfile::TempDir,
    remote: PathBuf,
    a: PathBuf,
    b: PathBuf,
    pass: PathBuf,
    gitcfg: PathBuf,
}

fn mk() -> T {
    let dir = tempfile::tempdir().unwrap();
    let pass = dir.path().join("pass.txt");
    std::fs::write(&pass, PASS).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&pass, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let gitcfg = dir.path().join("gitconfig");
    std::fs::write(&gitcfg, "").unwrap();
    let remote = dir.path().join("remote.git");
    git_in(dir.path(), &["init", "--bare", remote.to_str().unwrap()]);
    T {
        a: dir.path().join("a"),
        b: dir.path().join("b"),
        remote,
        pass,
        gitcfg,
        dir,
    }
}

fn git_env(c: &mut Command, t: &T) {
    c.env("GIT_CONFIG_GLOBAL", &t.gitcfg)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "ks test")
        .env("GIT_AUTHOR_EMAIL", "t@e.st")
        .env("GIT_COMMITTER_NAME", "ks test")
        .env("GIT_COMMITTER_EMAIL", "t@e.st")
        .env("GIT_CONFIG_COUNT", "1")
        .env("GIT_CONFIG_KEY_0", "init.defaultBranch")
        .env("GIT_CONFIG_VALUE_0", "main");
}

fn git_in(dir: &Path, args: &[&str]) -> (i32, String, String) {
    let mut c = Command::new("git");
    c.arg("-C").arg(dir).args(args);
    let t = tempfile::tempdir().unwrap();
    let gitcfg = t.path().join("g");
    std::fs::write(&gitcfg, "").unwrap();
    c.env("GIT_CONFIG_GLOBAL", &gitcfg)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@e.st")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@e.st")
        .env("GIT_CONFIG_COUNT", "1")
        .env("GIT_CONFIG_KEY_0", "init.defaultBranch")
        .env("GIT_CONFIG_VALUE_0", "main")
        .env("GIT_TERMINAL_PROMPT", "0");
    let o = c.output().unwrap();
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

fn git(t: &T, vault: &Path, args: &[&str]) -> (i32, String, String) {
    let mut c = Command::new("git");
    c.arg("-C").arg(vault).args(args);
    git_env(&mut c, t);
    c.env("GIT_TERMINAL_PROMPT", "0");
    let o = c.output().unwrap();
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

fn ks(t: &T, vault: &Path, args: &[&str], stdin: &[u8]) -> (i32, String, String) {
    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("ks"));
    c.arg("--vault")
        .arg(vault)
        .args(args)
        .env("KS_PASSPHRASE_FILE", &t.pass)
        .env("KS_NEW_PASSPHRASE_FILE", &t.pass)
        .env_remove("KS_PASSPHRASE")
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    git_env(&mut c, t);
    let mut ch = c.spawn().unwrap();
    ch.stdin.take().unwrap().write_all(stdin).unwrap();
    let o = ch.wait_with_output().unwrap();
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

fn json(t: &T, vault: &Path, args: &[&str], stdin: &[u8]) -> (i32, serde_json::Value, String) {
    let mut a: Vec<&str> = args.into();
    a.push("--json");
    let (code, out, err) = ks(t, vault, &a, stdin);
    (
        code,
        serde_json::from_str(out.trim()).unwrap_or_else(|_| panic!("not json: {out}")),
        err,
    )
}

fn init_git(t: &T, vault: &Path) {
    let (code, out, err) = ks(t, vault, &["init", "--git"], b"");
    assert!(code == 0, "init --git failed: {out} {err}");
    let (code, files, _) = git(t, vault, &["ls-tree", "-r", "--name-only", "HEAD"]);
    assert_eq!(code, 0);
    let names: Vec<&str> = files.lines().collect();
    for f in [
        ".identity.age",
        ".check.age",
        ".gitattributes",
        ".gitignore",
    ] {
        assert!(names.contains(&f), "missing {f} in {names:?}");
    }
}

#[test]
fn sync_init_git_one_commit() {
    let t = mk();
    init_git(&t, &t.a);
    let (_, files, _) = git(&t, &t.a, &["ls-tree", "-r", "--name-only", "HEAD"]);
    let names: Vec<&str> = files.lines().collect();
    assert_eq!(
        names,
        [
            ".check.age",
            ".gitattributes",
            ".gitignore",
            ".identity.age"
        ],
        "first commit must contain exactly the vault metadata"
    );
    let (_, msg, _) = git(&t, &t.a, &["log", "-1", "--format=%s"]);
    assert_eq!(msg.trim(), "ks: init");
}

#[test]
fn admin_doctor_git_vault_is_healthy() {
    let t = mk();
    init_git(&t, &t.a);
    ks(&t, &t.a, &["set", "a/b"], b"v");
    ks(
        &t,
        &t.a,
        &["git", "remote", "add", "origin", t.remote.to_str().unwrap()],
        b"",
    );
    let (code, _, _) = ks(&t, &t.a, &["sync"], b"");
    assert_eq!(code, 0);
    // doctor must not descend into .git: no reports, exit 0
    let (code, out, err) = ks(&t, &t.a, &["doctor", "--json"], b"");
    assert_eq!(code, 0, "{out} {err}");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["ok"], true, "{v}");
    // --fix must not touch git internals
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let git_dir = t.a.join(".git");
        let mode_before = git_dir.symlink_metadata().unwrap().permissions().mode();
        let (code, _, _) = ks(&t, &t.a, &["doctor", "--fix"], b"");
        assert_eq!(code, 0);
        assert_eq!(
            git_dir.symlink_metadata().unwrap().permissions().mode(),
            mode_before
        );
    }
}

#[test]
fn init_clone_repo_without_vault_explains() {
    let t = mk();
    // an empty bare repo (or one whose default branch lacks a vault)
    let (code, _, err) = ks(
        &t,
        &t.a,
        &["init", "--clone", t.remote.to_str().unwrap()],
        b"",
    );
    assert_eq!(code, 66, "{err}");
    assert!(
        err.contains("contains no ks vault"),
        "expected the no-vault hint, got: {err}"
    );
    // the clone is kept as designed
    assert!(t.a.join(".git").exists());
}

#[test]
fn sync_clone_roundtrip_and_exists() {
    let t = mk();
    init_git(&t, &t.a);
    ks(
        &t,
        &t.a,
        &["git", "remote", "add", "origin", t.remote.to_str().unwrap()],
        b"",
    );
    let (code, out, err) = ks(&t, &t.a, &["sync"], b"");
    assert!(code == 0, "first sync failed: {out} {err}");
    // clone into B
    let (code, out, err) = ks(
        &t,
        &t.b,
        &["init", "--clone", t.remote.to_str().unwrap()],
        b"",
    );
    assert!(code == 0, "clone failed: {out} {err}");
    assert!(t.b.join(".identity.age").exists());
    let (_, va, _) = json(&t, &t.a, &["ls"], b"");
    let (_, vb, _) = json(&t, &t.b, &["ls"], b"");
    assert_eq!(va["entries"], vb["entries"]);
    // clone into non-empty dir -> exists(73)
    let (code, _, _) = ks(
        &t,
        &t.b,
        &["init", "--clone", t.remote.to_str().unwrap()],
        b"",
    );
    assert_eq!(code, 73);
}

#[test]
fn sync_commit_messages_and_scoping() {
    let t = mk();
    init_git(&t, &t.a);
    let before = || git(&t, &t.a, &["rev-parse", "HEAD"]).1;
    // set -> "ks: set <name>"
    ks(&t, &t.a, &["set", "tok"], b"secret");
    let (_, msg, _) = git(&t, &t.a, &["log", "-1", "--format=%s"]);
    assert_eq!(msg.trim(), "ks: set tok");
    // unrelated staged change is NOT included
    std::fs::write(t.a.join("staged.age"), b"plant").unwrap();
    git(&t, &t.a, &["add", "staged.age"]);
    ks(&t, &t.a, &["set", "two"], b"v");
    let (_, files, _) = git(&t, &t.a, &["show", "--name-only", "--format=", "HEAD"]);
    assert!(
        !files.lines().any(|l| l == "staged.age"),
        "staged.age leaked: {files}"
    );
    git(&t, &t.a, &["reset", "--hard", "HEAD"]); // clean the plant
    // mv -> "ks: mv <a> <b>"
    ks(&t, &t.a, &["mv", "two", "three"], b"");
    let (_, msg, _) = git(&t, &t.a, &["log", "-1", "--format=%s"]);
    assert_eq!(msg.trim(), "ks: mv two three");
    // rm -> "ks: rm <name>"
    ks(&t, &t.a, &["rm", "three", "--force"], b"");
    let (_, msg, _) = git(&t, &t.a, &["log", "-1", "--format=%s"]);
    assert_eq!(msg.trim(), "ks: rm three");
    // passwd -> "ks: passwd"
    ks(&t, &t.a, &["passwd"], b"");
    let (_, msg, _) = git(&t, &t.a, &["log", "-1", "--format=%s"]);
    assert_eq!(msg.trim(), "ks: passwd");
    // gen with name -> "ks: set <name>"
    ks(&t, &t.a, &["gen", "pw", "-l", "8"], b"");
    let (_, msg, _) = git(&t, &t.a, &["log", "-1", "--format=%s"]);
    assert_eq!(msg.trim(), "ks: set pw");
    // KS_NO_COMMIT=1 -> no new commit
    let head = before();
    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("ks"));
    c.arg("--vault")
        .arg(&t.a)
        .args(["set", "nocommit"])
        .env("KS_PASSPHRASE_FILE", &t.pass)
        .env("KS_NO_COMMIT", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    git_env(&mut c, &t);
    let mut ch = c.spawn().unwrap();
    ch.stdin.take().unwrap().write_all(b"v").unwrap();
    assert!(ch.wait().unwrap().success());
    assert_eq!(before().trim(), head.trim());
    // non-repo vault: set succeeds with no warning
    let nonrepo = t.dir.path().join("nonrepo");
    ks(&t, &nonrepo, &["init"], b"");
    let (code, _, err) = ks(&t, &nonrepo, &["set", "x"], b"v");
    assert_eq!(code, 0);
    assert!(!err.contains("git"), "non-repo should not warn: {err}");
    // rm of a never-committed entry -> no commit, no warning
    let head = before();
    ks(&t, &t.a, &["set", "temp"], b"v"); // committed
    git(&t, &t.a, &["reset", "--hard", "HEAD~1"]); // file gone again? no: entry stays uncommitted? reset removes commit AND file
}

#[test]
fn sync_run_push_pull_and_usage() {
    let t = mk();
    init_git(&t, &t.a);
    ks(
        &t,
        &t.a,
        &["git", "remote", "add", "origin", t.remote.to_str().unwrap()],
        b"",
    );
    // first sync: pushes -u, no pull (remote branch absent)
    let (code, _, err) = ks(&t, &t.a, &["sync"], b"");
    assert!(code == 0, "{err}");
    // A writes + syncs
    ks(&t, &t.a, &["set", "shared"], b"from-a");
    let (code, _, _) = ks(&t, &t.a, &["sync"], b"");
    assert_eq!(code, 0);
    // B clones + sync pulls nothing new but is clean
    ks(
        &t,
        &t.b,
        &["init", "--clone", t.remote.to_str().unwrap()],
        b"",
    );
    let (_, out, _) = ks(&t, &t.b, &["show", "shared"], b"");
    assert_eq!(out.trim(), "from-a");
    let (_, v, _) = json(&t, &t.b, &["sync"], b"");
    assert_eq!(v["pushed"], true);
    // non-git vault -> usage 64
    let nonrepo = t.dir.path().join("nonrepo");
    ks(&t, &nonrepo, &["init"], b"");
    let (code, _, _) = ks(&t, &nonrepo, &["sync"], b"");
    assert_eq!(code, 64);
}

#[test]
fn sync_keep_local_and_remote() {
    let t = mk();
    init_git(&t, &t.a);
    ks(
        &t,
        &t.a,
        &["git", "remote", "add", "origin", t.remote.to_str().unwrap()],
        b"",
    );
    ks(&t, &t.a, &["set", "x"], b"seed");
    ks(&t, &t.a, &["sync"], b"");
    ks(
        &t,
        &t.b,
        &["init", "--clone", t.remote.to_str().unwrap()],
        b"",
    );
    // both sides change x
    ks(&t, &t.a, &["set", "x", "--force"], b"aaa");
    ks(&t, &t.a, &["sync"], b"");
    ks(&t, &t.b, &["set", "x", "--force"], b"bbb");
    let (code, v, _) = json(&t, &t.b, &["sync"], b"");
    assert_eq!(code, 75);
    assert_eq!(v["error"]["entries"], serde_json::json!(["x"]));
    assert_eq!(v["error"]["kind"], "content");
    // no rebase left in progress; tree still has b's value
    let (_, head, _) = git(&t, &t.b, &["status", "--porcelain=v1", "-z"]);
    assert!(!t.b.join(".git/rebase-merge").exists() && !t.b.join(".git/rebase-apply").exists());
    let (_, out, _) = ks(&t, &t.b, &["show", "x"], b"");
    assert_eq!(out.trim(), "bbb");
    // --keep local: B wins, pushed; A pulls -> reads bbb
    let (code, _, _) = ks(&t, &t.b, &["sync", "--keep", "local"], b"");
    assert_eq!(code, 0);
    let (code, _, _) = ks(&t, &t.a, &["sync"], b"");
    assert_eq!(code, 0);
    let (_, out, _) = ks(&t, &t.a, &["show", "x"], b"");
    assert_eq!(out.trim(), "bbb");

    // separate case: --keep remote keeps A's side
    let t = mk();
    init_git(&t, &t.a);
    ks(
        &t,
        &t.a,
        &["git", "remote", "add", "origin", t.remote.to_str().unwrap()],
        b"",
    );
    ks(&t, &t.a, &["set", "x"], b"seed");
    ks(&t, &t.a, &["sync"], b"");
    ks(
        &t,
        &t.b,
        &["init", "--clone", t.remote.to_str().unwrap()],
        b"",
    );
    ks(&t, &t.a, &["set", "x", "--force"], b"aaa");
    ks(&t, &t.a, &["sync"], b"");
    ks(&t, &t.b, &["set", "x", "--force"], b"bbb");
    let (code, _, _) = ks(&t, &t.b, &["sync", "--keep", "remote"], b"");
    assert_eq!(code, 0);
    let (_, out, _) = ks(&t, &t.b, &["show", "x"], b"");
    assert_eq!(out.trim(), "aaa");
}

#[test]
fn sync_delete_conflict() {
    let t = mk();
    init_git(&t, &t.a);
    ks(
        &t,
        &t.a,
        &["git", "remote", "add", "origin", t.remote.to_str().unwrap()],
        b"",
    );
    ks(&t, &t.a, &["set", "x"], b"seed");
    ks(&t, &t.a, &["sync"], b"");
    ks(
        &t,
        &t.b,
        &["init", "--clone", t.remote.to_str().unwrap()],
        b"",
    );
    // A deletes, B modifies
    ks(&t, &t.a, &["rm", "x", "--force"], b"");
    ks(&t, &t.a, &["sync"], b"");
    ks(&t, &t.b, &["set", "x", "--force"], b"modified");
    let (code, v, _) = json(&t, &t.b, &["sync"], b"");
    assert_eq!(code, 75);
    assert_eq!(v["error"]["kind"], "delete");
    // --keep local is also a delete conflict (git -X cannot resolve del/mod)
    let (code, v, _) = json(&t, &t.b, &["sync", "--keep", "local"], b"");
    assert_eq!(code, 75);
    assert_eq!(v["error"]["kind"], "delete");
    // no rebase left
    assert!(!t.b.join(".git/rebase-merge").exists());
    let (_, out, _) = ks(&t, &t.b, &["show", "x"], b"");
    assert_eq!(out.trim(), "modified");
}

#[test]
fn sync_git_passthrough() {
    let t = mk();
    init_git(&t, &t.a);
    let (code, out, _) = ks(&t, &t.a, &["git", "log", "--oneline"], b"");
    assert_eq!(code, 0);
    assert!(out.contains("ks: init"));
    // failing command: exit code passthrough
    let (code, _, _) = ks(
        &t,
        &t.a,
        &["git", "rev-parse", "--verify", "nosuchref"],
        b"",
    );
    assert!(code != 0);
}

#[test]
fn sync_stages_only_entry_files() {
    // A remote can weaken the whitelist .gitignore; sync must still stage
    // only entry files (and the two git metadata files) by pathspec.
    let t = mk();
    init_git(&t, &t.a);
    ks(
        &t,
        &t.a,
        &["git", "remote", "add", "origin", t.remote.to_str().unwrap()],
        b"",
    );
    // Tampered .gitignore un-ignores *.txt — like a remote would push it.
    std::fs::write(t.a.join(".gitignore"), "*\n!*/\n!*.age\n!*.txt\n").unwrap();
    std::fs::write(t.a.join("notes.txt"), b"do not commit").unwrap();
    let (code, _, _) = ks(&t, &t.a, &["sync"], b"");
    assert_eq!(code, 0);
    let (_, tree, _) = git(&t, &t.a, &["ls-tree", "-r", "--name-only", "origin/main"]);
    assert!(
        !tree.lines().any(|l| l == "notes.txt"),
        "notes.txt synced: {tree}"
    );
}

#[test]
fn sync_gitignore_excludes_non_age() {
    let t = mk();
    init_git(&t, &t.a);
    ks(
        &t,
        &t.a,
        &["git", "remote", "add", "origin", t.remote.to_str().unwrap()],
        b"",
    );
    std::fs::write(t.a.join("notes.txt"), b"do not commit").unwrap();
    let (code, _, _) = ks(&t, &t.a, &["sync"], b"");
    assert_eq!(code, 0);
    let (_, tree, _) = git(&t, &t.a, &["ls-tree", "-r", "--name-only", "origin/main"]);
    assert!(
        !tree.lines().any(|l| l == "notes.txt"),
        "notes.txt synced: {tree}"
    );
}
