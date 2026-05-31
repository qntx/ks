//! Integration tests for the agent-facing `--json` interface.
//!
//! Spawns the real `ks` binary against a throwaway `KS_DIR` with a fixed
//! `KS_PASSPHRASE`, and asserts that each command emits well-formed JSON that
//! matches the documented schema.
#![allow(
    unused_crate_dependencies,
    clippy::indexing_slicing,
    clippy::missing_assert_message,
    clippy::tests_outside_test_module,
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "integration test harness: it only links serde_json; panicking JSON indexing, terse asserts, top-level #[test] fns, and expect/unwrap on failure are intentional"
)]

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

const PASS: &str = "integration-pass-123456";

fn unique_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("ks-json-it-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn run(dir: &Path, args: &[&str], stdin: Option<&[u8]>) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ks"));
    cmd.args(args)
        .env("KS_DIR", dir)
        .env("KS_PASSPHRASE", PASS)
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().expect("spawn ks");
    if let Some(bytes) = stdin {
        child
            .stdin
            .take()
            .expect("stdin")
            .write_all(bytes)
            .expect("write stdin");
    }
    child.wait_with_output().expect("wait ks")
}

fn json(dir: &Path, args: &[&str], stdin: Option<&[u8]>) -> serde_json::Value {
    let out = run(dir, args, stdin);
    assert!(
        out.status.success(),
        "command {args:?} failed: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    serde_json::from_slice(&out.stdout).expect("stdout is valid JSON")
}

#[test]
fn json_roundtrip_and_error_contract() {
    let dir = unique_dir();

    let init = json(&dir, &["--json", "init"], None);
    assert!(
        init["public_key"]
            .as_str()
            .is_some_and(|k| k.starts_with("age1"))
    );

    let inserted = json(
        &dir,
        &["--json", "insert", "svc/token", "--multiline"],
        Some(b"ghp_xxx\nuser: alice\n"),
    );
    assert_eq!(inserted["path"], "svc/token");
    assert_eq!(inserted["stored"], serde_json::Value::Bool(true));

    let shown = json(&dir, &["--json", "show", "svc/token"], None);
    assert_eq!(shown["kind"], "text");
    assert_eq!(shown["value"], "ghp_xxx");
    assert_eq!(shown["fields"]["user"], "alice");

    let field = json(&dir, &["--json", "show", "svc/token", "-f", "user"], None);
    assert_eq!(field["value"], "alice");

    let listed = json(&dir, &["--json", "ls"], None);
    assert_eq!(listed["secrets"][0], "svc/token");

    let generated = json(&dir, &["--json", "gen", "-l", "16"], None);
    assert_eq!(generated["length"], 16);
    assert_eq!(generated["value"].as_str().map(str::len), Some(16));

    // Destructive op without --force is rejected with a JSON error + non-zero exit.
    let out = run(&dir, &["--json", "rm", "svc/token"], None);
    assert!(!out.status.success());
    let err: serde_json::Value = serde_json::from_slice(&out.stdout).expect("error JSON");
    assert!(err["error"].is_string());

    // With --force it succeeds.
    let removed = json(&dir, &["--json", "rm", "svc/token", "--force"], None);
    assert_eq!(removed["removed"], serde_json::Value::Bool(true));
}

#[test]
fn json_unlock_requires_passphrase() {
    let dir = unique_dir();
    json(&dir, &["--json", "init"], None);

    // Reading without KS_PASSPHRASE in --json mode must fail with a JSON error,
    // never hang on a prompt.
    let out = Command::new(env!("CARGO_BIN_EXE_ks"))
        .args(["--json", "show", "any"])
        .env("KS_DIR", &dir)
        .env_remove("KS_PASSPHRASE")
        .stdin(Stdio::null())
        .output()
        .expect("run ks");
    assert!(!out.status.success());
    let err: serde_json::Value = serde_json::from_slice(&out.stdout).expect("error JSON");
    assert!(err["error"].as_str().unwrap().contains("KS_PASSPHRASE"));
}
