//! Tests that chmod the SHARED agent endpoint directory
//! (`~/Library/Application Support/ks/run` on macOS).
//!
//! Every vault's socket lives under the same `run/` dir, so a test that
//! loosens its permissions breaks any concurrently running agent test:
//! `__agent` can no longer bind ("agent did not become ready") and
//! clients probing the endpoint take error paths they would not take
//! otherwise. Cargo runs test binaries sequentially but tests inside a
//! binary in parallel, so these mutators live in their own binary and
//! serialize on a static mutex.
#![allow(
    unused_crate_dependencies,
    dead_code,
    unused_imports,
    clippy::tests_outside_test_module,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::missing_assert_message,
    clippy::items_after_statements,
    reason = "integration test harness: panicking on setup and terse assertions are intentional"
)]

use std::io::Write as _;
use std::process::{Command, Stdio};
use std::sync::Mutex;

const PASS: &str = "correct horse battery staple fixture";

/// Serializes the two mutators against each other.
static RUN_DIR: Mutex<()> = Mutex::new(());

struct T {
    /// Keeps the temporary directory alive; the vault/pass paths into
    /// it are what tests use.
    #[allow(dead_code, reason = "RAII field: keeps TempDir alive")]
    dir: tempfile::TempDir,
    vault: std::path::PathBuf,
    pass: std::path::PathBuf,
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
    T {
        vault: dir.path().join("v"),
        pass,
        dir,
    }
}

fn ks(t: &T, args: &[&str]) -> Command {
    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("ks"));
    c.arg("--vault")
        .arg(&t.vault)
        .args(args)
        .env("KS_PASSPHRASE_FILE", &t.pass)
        .env("KS_NEW_PASSPHRASE_FILE", &t.pass)
        .env("GIT_TERMINAL_PROMPT", "0");
    c
}

fn run(t: &T, args: &[&str], stdin: &[u8]) -> (i32, String, String) {
    let mut ch = ks(t, args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    ch.stdin.take().unwrap().write_all(stdin).unwrap();
    let o = ch.wait_with_output().unwrap();
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

fn init(t: &T) {
    let (code, out, err) = run(t, &["init"], b"");
    assert!(code == 0, "init failed: {out} {err}");
}

/// Locks the test's agent on drop so daemons never outlive the test.
struct Guard<'a>(&'a T);
impl Drop for Guard<'_> {
    fn drop(&mut self) {
        let _ = run(self.0, &["lock"], b"");
    }
}

#[cfg(unix)]
#[test]
fn agent_isolation_endpoint_permissions() {
    let _serial = RUN_DIR.lock().unwrap();
    let t = mk();
    init(&t);
    let _g = Guard(&t);
    let (code, out, err) = run(&t, &["unlock", "--ttl", "60"], b"");
    assert!(code == 0, "unlock failed: {out} {err}");
    let ep = ks::agent::endpoint(&ks::Vault::open(&t.vault).unwrap()).unwrap();
    let path = ep.path().unwrap();
    use std::os::unix::fs::PermissionsExt as _;
    let dir_meta = std::fs::metadata(path.parent().unwrap()).unwrap();
    use std::os::unix::fs::MetadataExt as _;
    assert_eq!(dir_meta.mode() & 0o777, 0o700);
    std::fs::set_permissions(
        path.parent().unwrap(),
        std::fs::Permissions::from_mode(0o777),
    )
    .unwrap();
    assert!(ks::agent::connect_raw(&ks::Vault::open(&t.vault).unwrap()).is_err());
    std::fs::set_permissions(
        path.parent().unwrap(),
        std::fs::Permissions::from_mode(0o700),
    )
    .unwrap();
}

#[cfg(unix)]
#[test]
fn unlock_soft_fails_on_loose_state_dir() {
    use std::os::unix::fs::PermissionsExt as _;
    let _serial = RUN_DIR.lock().unwrap();
    let t = mk();
    init(&t);
    run(&t, &["set", "a"], b"v");
    // Loosen the directory holding the agent endpoint. Linux: a private
    // XDG_RUNTIME_DIR owned by this test. macOS: the shared run dir under
    // Application Support (directories ignores env), restored afterwards.
    let xdg = tempfile::tempdir().unwrap();
    let dir = if cfg!(target_os = "macos") {
        std::env::home_dir()
            .unwrap()
            .join("Library/Application Support/ks/run")
    } else {
        xdg.path().join("ks")
    };
    std::fs::create_dir_all(&dir).unwrap();
    let mode = dir.symlink_metadata().unwrap().permissions().mode() & 0o777;
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    let with_pass = ks(&t, &["show", "a"])
        .env("XDG_RUNTIME_DIR", xdg.path())
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let without = ks(&t, &["show", "a"])
        .env("XDG_RUNTIME_DIR", xdg.path())
        .env_remove("KS_PASSPHRASE_FILE")
        .env_remove("KS_PASSPHRASE")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(mode)).unwrap();
    let err = String::from_utf8_lossy(&with_pass.stderr);
    assert_eq!(with_pass.status.code(), Some(0), "{err}");
    assert!(err.contains("agent"), "expected an agent warning: {err}");
    assert_eq!(without.status.code(), Some(77));
}
