//! M5 key-rotation tests (ADMIN-REKEY*, DEVICE-ROLLBACK). Resume coverage
//! lives in the library unit tests: the two-key interrupted states can
//! only be built with pub(crate) helpers, so the CLI tests cover fresh
//! rekey, the rollback path, and resume via the library tests.

#![allow(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::missing_assert_message,
    clippy::let_underscore_must_use,
    clippy::items_after_statements,
    clippy::shadow_unrelated,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation,
    clippy::assert_is_empty,
    clippy::string_lit_as_bytes,
    clippy::cognitive_complexity,
    reason = "integration test harness: panicking fixtures and terse assertions are intentional"
)]

use std::io::Write as _;
use std::process::{Command, Stdio};

const PASS: &str = "correct horse battery staple fixture";
const NEW_PASS: &str = "a completely different new passphrase";

struct T {
    vault: std::path::PathBuf,
    pass: std::path::PathBuf,
    new_pass: std::path::PathBuf,
    gitcfg: std::path::PathBuf,
    _dir: tempfile::TempDir,
}

fn mk() -> T {
    let dir = tempfile::tempdir().unwrap();
    let pass = dir.path().join("pass.txt");
    std::fs::write(&pass, PASS).unwrap();
    let new_pass = dir.path().join("newpass.txt");
    std::fs::write(&new_pass, NEW_PASS).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&pass, std::fs::Permissions::from_mode(0o600)).unwrap();
        std::fs::set_permissions(&new_pass, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let gitcfg = dir.path().join("gitconfig");
    std::fs::write(&gitcfg, "").unwrap();
    T {
        vault: dir.path().join("v"),
        pass,
        new_pass,
        gitcfg,
        _dir: dir,
    }
}

/// Keeps git away from the developer's and the runner's configuration
/// (identity, signing, hooks) so commits made by `ks` are deterministic.
fn isolate_git(c: &mut Command, gitcfg: &std::path::Path) {
    c.env("GIT_CONFIG_GLOBAL", gitcfg)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "ks test")
        .env("GIT_AUTHOR_EMAIL", "t@e.st")
        .env("GIT_COMMITTER_NAME", "ks test")
        .env("GIT_COMMITTER_EMAIL", "t@e.st")
        .env("GIT_CONFIG_COUNT", "1")
        .env("GIT_CONFIG_KEY_0", "init.defaultBranch")
        .env("GIT_CONFIG_VALUE_0", "main");
}

fn ks(t: &T, args: &[&str], stdin: &[u8], pass: Option<&std::path::Path>) -> (i32, String, String) {
    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("ks"));
    isolate_git(&mut c, &t.gitcfg);
    c.arg("--vault")
        .arg(&t.vault)
        .args(args)
        .env_remove("KS_PASSPHRASE")
        .env_remove("KS_PASSPHRASE_FILE")
        .env_remove("KS_NEW_PASSPHRASE_FILE")
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(p) = pass {
        c.env("KS_PASSPHRASE_FILE", p)
            .env("KS_NEW_PASSPHRASE_FILE", p);
    }
    let mut ch = c.spawn().unwrap();
    ch.stdin.take().unwrap().write_all(stdin).unwrap();
    let o = ch.wait_with_output().unwrap();
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

fn init(t: &T) {
    let (code, out, err) = ks(t, &["init", "--git"], b"", Some(&t.pass));
    assert!(code == 0, "init failed: {out} {err}");
}

fn set(t: &T, n: &str, v: &str) {
    let (code, out, err) = ks(t, &["set", n], v.as_bytes(), Some(&t.pass));
    assert!(code == 0, "set {n} failed: {out} {err}");
}

/// Decrypts `.identity.age` with `pass` via the `age` crate alone.
fn open_identity_text(t: &T, pass: &str) -> String {
    let bytes = std::fs::read(t.vault.join(".identity.age")).unwrap();
    let decryptor = age::Decryptor::new_buffered(&bytes[..]).unwrap();
    let mut scrypt = age::scrypt::Identity::new(age::secrecy::SecretString::from(pass));
    scrypt.set_max_work_factor(20);
    let scrypt_ref: &dyn age::Identity = &scrypt;
    let mut reader = decryptor.decrypt(std::iter::once(scrypt_ref)).unwrap();
    let mut text = String::new();
    std::io::Read::read_to_string(&mut reader, &mut text).unwrap();
    text
}

/// Decrypts a vault file under the identities parsed from `identity_text`.
fn decrypt_with_identity(bytes: &[u8], identity_text: &str) -> bool {
    let keys: Vec<age::x25519::Identity> = identity_text
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.parse().unwrap())
        .collect();
    let Ok(decryptor) = age::Decryptor::new_buffered(bytes) else {
        return false;
    };
    decryptor
        .decrypt(keys.iter().map(|k| {
            let r: &dyn age::Identity = k;
            r
        }))
        .is_ok()
}

#[test]
fn admin_rekey_full_cycle() {
    let t = mk();
    init(&t);
    set(&t, "a/b", "v1");
    set(&t, "c", "v2");
    // agent running + no touchid/keychain
    let (code, _, err) = ks(&t, &["unlock"], b"", Some(&t.pass));
    assert_eq!(code, 0, "unlock failed: {err}");
    let old_identity = open_identity_text(&t, PASS);

    // rekey with the passphrase source + a new-passphrase file
    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("ks"));
    isolate_git(&mut c, &t.gitcfg);
    c.arg("--vault")
        .arg(&t.vault)
        .arg("rekey")
        .env_remove("KS_PASSPHRASE")
        .env("KS_PASSPHRASE_FILE", &t.pass)
        .env("KS_NEW_PASSPHRASE_FILE", &t.new_pass)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let o = c.output().unwrap();
    assert_eq!(
        o.status.code(),
        Some(0),
        "rekey: {}",
        String::from_utf8_lossy(&o.stderr)
    );

    // old passphrase is dead; new works
    let (code, _, err) = ks(&t, &["show", "a/b"], b"", Some(&t.pass));
    assert_ne!(code, 0, "old passphrase should fail: {err}");
    let (code, out, err) = ks(&t, &["show", "a/b"], b"", Some(&t.new_pass));
    assert_eq!(code, 0, "new passphrase failed: {err}");
    assert!(out.contains("v1"), "{out}");

    // the OLD identity (opened via the age crate alone) decrypts nothing now
    for n in ["a/b.age", "c.age", ".check.age"] {
        let bytes = std::fs::read(t.vault.join(n)).unwrap();
        assert!(
            !decrypt_with_identity(&bytes, &old_identity),
            "old identity still reads {n}"
        );
    }
    // agent is gone
    let (code, out, _) = ks(&t, &["status", "--json"], b"", None);
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert!(v["agent"].is_null(), "{out}");
    // git commit covering all rewritten paths
    let log = Command::new("git")
        .arg("-C")
        .arg(&t.vault)
        .args(["log", "--oneline"])
        .output()
        .unwrap();
    let log = String::from_utf8_lossy(&log.stdout);
    assert!(log.contains("ks: rekey"), "log: {log}");
    let show = Command::new("git")
        .arg("-C")
        .arg(&t.vault)
        .args(["show", "--name-only", "--format=", "HEAD"])
        .output()
        .unwrap();
    let files = String::from_utf8_lossy(&show.stdout);
    for f in ["a/b.age", "c.age", ".identity.age", ".check.age"] {
        assert!(files.contains(f), "missing {f} in commit: {files}");
    }
}

/// DEVICE-ROLLBACK: a restored pre-rekey check must not let an impostor
/// agent (holding the OLD identity) unlock; a passphrase unlock detects
/// the stale source, warns, removes it, and heals the check.
#[test]
fn device_rollback_restored_check_not_trusted() {
    let t = mk();
    init(&t);
    set(&t, "a/b", "v1");
    let old_identity = open_identity_text(&t, PASS);
    let old_check = std::fs::read(t.vault.join(".check.age")).unwrap();
    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("ks"));
    isolate_git(&mut c, &t.gitcfg);
    c.arg("--vault")
        .arg(&t.vault)
        .arg("rekey")
        .env_remove("KS_PASSPHRASE")
        .env("KS_PASSPHRASE_FILE", &t.pass)
        .env("KS_NEW_PASSPHRASE_FILE", &t.new_pass)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    assert_eq!(c.output().unwrap().status.code(), Some(0));
    std::fs::write(t.vault.join(".check.age"), &old_check).unwrap();
    let mut agent = Command::new(assert_cmd::cargo::cargo_bin!("ks"))
        .arg("--vault")
        .arg(&t.vault)
        .args(["__agent", "--ttl", "60", "--max", "300"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    agent
        .stdin
        .take()
        .unwrap()
        .write_all(old_identity.as_bytes())
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let (_, out, _) = ks(&t, &["status", "--json"], b"", None);
        if serde_json::from_str::<serde_json::Value>(&out).is_ok_and(|v| !v["agent"].is_null()) {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "impostor agent did not come up"
        );
    }
    // no passphrase -> locked (the stale agent is not trusted)
    let (code, _, _) = ks(&t, &["show", "a/b"], b"", None);
    assert_eq!(code, 77);
    // passphrase -> works, warns about the agent, agent exits
    let (code, out, err) = ks(&t, &["show", "a/b"], b"", Some(&t.new_pass));
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("v1"));
    assert!(
        err.contains("agent") || err.contains("check"),
        "expected stale-source warning: {err}"
    );
    let _ = agent.wait();
    let (code, out, _) = ks(&t, &["status", "--json"], b"", None);
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert!(v["agent"].is_null(), "agent still running: {out}");
}

#[test]
fn admin_rekey_same_passphrase_rejected() {
    let t = mk();
    init(&t);
    set(&t, "a", "v");
    let before = std::fs::read(t.vault.join(".identity.age")).unwrap();
    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("ks"));
    isolate_git(&mut c, &t.gitcfg);
    c.arg("--vault")
        .arg(&t.vault)
        .arg("rekey")
        .env_remove("KS_PASSPHRASE")
        .env("KS_PASSPHRASE_FILE", &t.pass)
        .env("KS_NEW_PASSPHRASE_FILE", &t.pass)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let o = c.output().unwrap();
    assert_eq!(o.status.code(), Some(65));
    assert!(
        String::from_utf8_lossy(&o.stderr).contains("differ"),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    assert_eq!(
        std::fs::read(t.vault.join(".identity.age")).unwrap(),
        before
    );
}

#[test]
fn admin_rekey_dirty_tree_rejected() {
    let t = mk();
    init(&t);
    set(&t, "a", "v");
    std::fs::write(t.vault.join("dirty.age"), b"not-an-entry").unwrap();
    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("ks"));
    isolate_git(&mut c, &t.gitcfg);
    c.arg("--vault")
        .arg(&t.vault)
        .arg("rekey")
        .env_remove("KS_PASSPHRASE")
        .env("KS_PASSPHRASE_FILE", &t.pass)
        .env("KS_NEW_PASSPHRASE_FILE", &t.new_pass)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let o = c.output().unwrap();
    assert_eq!(o.status.code(), Some(64));
    std::fs::remove_file(t.vault.join("dirty.age")).unwrap();
}

#[test]
fn admin_rekey_undecryptable_aborts() {
    let t = mk();
    // non-git vault: a planted file would otherwise fail the dirty check
    let (code, out, err) = ks(&t, &["init"], b"", Some(&t.pass));
    assert!(code == 0, "init failed: {out} {err}");
    set(&t, "a", "v");
    // a valid single-X25519 age file to a foreign recipient
    let foreign = age::x25519::Identity::generate().to_public();
    let recipient: &dyn age::Recipient = &foreign;
    let encryptor = age::Encryptor::with_recipients(std::iter::once(recipient)).unwrap();
    let mut out = Vec::new();
    let mut w = encryptor.wrap_output(&mut out).unwrap();
    std::io::Write::write_all(&mut w, b"ks1 text rogue\nx").unwrap();
    w.finish().unwrap();
    std::fs::write(t.vault.join("rogue.age"), &out).unwrap();
    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("ks"));
    isolate_git(&mut c, &t.gitcfg);
    c.arg("--vault")
        .arg(&t.vault)
        .arg("rekey")
        .env_remove("KS_PASSPHRASE")
        .env("KS_PASSPHRASE_FILE", &t.pass)
        .env("KS_NEW_PASSPHRASE_FILE", &t.new_pass)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let o = c.output().unwrap();
    assert_eq!(o.status.code(), Some(65));
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(err.contains("rogue"), "missing entry name: {err}");
    // nothing changed: old passphrase still works
    let (code, out, _) = ks(&t, &["show", "a"], b"", Some(&t.pass));
    assert_eq!(code, 0, "{out}");
    std::fs::remove_file(t.vault.join("rogue.age")).unwrap();
}
