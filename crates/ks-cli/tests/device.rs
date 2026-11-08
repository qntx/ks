//! M4 device-unlock tests (DEVICE-*). Touch ID needs a human for the real
//! biometrics path, so the automated coverage is the missing-plugin path
//! and the keychain happy/unavailable paths plus status JSON.

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
    reason = "integration test harness: panicking fixtures and terse assertions are intentional"
)]

use std::io::Write as _;
use std::process::{Command, Stdio};

const PASS: &str = "correct horse battery staple fixture";

struct T {
    _dir: tempfile::TempDir,
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
        _dir: dir,
    }
}

fn ks(t: &T, args: &[&str], stdin: &[u8], with_pass: bool) -> (i32, String, String) {
    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("ks"));
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
    if with_pass {
        c.env("KS_PASSPHRASE_FILE", &t.pass)
            .env("KS_NEW_PASSPHRASE_FILE", &t.pass);
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
    let (code, out, err) = ks(t, &["init"], b"", true);
    assert!(code == 0, "init failed: {out} {err}");
}

/// A PATH that does not contain `age-plugin-se`, keeping the rest of the
/// harness environment functional.
fn path_without_se() -> std::ffi::OsString {
    let dir = tempfile::tempdir().unwrap();
    // keep the dir alive for the child: leaked tempdir is fine in tests
    std::ffi::OsString::from(dir.keep())
}

#[test]
fn device_touchid_missing_plugin() {
    let t = mk();
    init(&t);
    // enable with a PATH lacking age-plugin-se -> unavailable (69) + hint
    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("ks"));
    c.arg("--vault")
        .arg(&t.vault)
        .args(["touchid", "enable"])
        .env("PATH", path_without_se())
        .env("KS_PASSPHRASE_FILE", &t.pass)
        .env_remove("KS_PASSPHRASE")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let o = c.output().unwrap();
    assert_eq!(
        o.status.code(),
        Some(69),
        "stdout: {}",
        String::from_utf8_lossy(&o.stdout)
    );
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(err.contains("age-plugin-se"), "missing hint: {err}");
    // status reports disabled without unlocking
    let (code, out, _) = ks(&t, &["status", "--json"], b"", false);
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["touchid"], false);
}

#[test]
fn device_keychain_enable_disable() {
    let t = mk();
    init(&t);
    let (code, out, err) = ks(&t, &["keychain", "enable", "--json"], b"", true);
    // Platform-dependent: the store may be absent (CI without Secret
    // Service, locked login keychain) — accept only 0 or 69, and the JSON
    // must be well formed either way.
    assert!(
        code == 0 || code == 69,
        "unexpected exit {code}: {out} {err}"
    );
    if code == 0 {
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["enabled"], true);
        // status reflects it
        let (code, out, _) = ks(&t, &["status", "--json"], b"", false);
        assert_eq!(code, 0);
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["keychain"], true);
        // always clean up
        let (code, out, _) = ks(&t, &["keychain", "disable", "--json"], b"", false);
        assert_eq!(code, 0, "{out}");
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["was_enabled"], true);
    }
    // disable is idempotent
    let (code, out, _) = ks(&t, &["keychain", "disable", "--json"], b"", false);
    assert_eq!(code, 0, "{out}");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["was_enabled"], false);
}

#[test]
fn device_status_shape() {
    let t = mk();
    init(&t);
    let (code, out, _) = ks(&t, &["status", "--json"], b"", false);
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert!(v["touchid"].is_boolean(), "{out}");
    assert!(v["keychain"].is_boolean(), "{out}");
    assert!(v["git"]["repo"].is_boolean(), "{out}");
}
