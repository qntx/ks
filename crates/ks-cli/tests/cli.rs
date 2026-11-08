//! M1 CLI integration tests: one or more tests per M1 spec ID, named after it.
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
    unused_variables,
    clippy::excessive_nesting,
    clippy::items_after_statements,
    clippy::shadow_unrelated,
    reason = "integration test harness: panicking on setup and terse assertions are intentional"
)]

use std::io::Write as _;
use std::path::PathBuf;
use std::process::{Command, Stdio};

const PASS: &str = "correct horse battery staple fixture";
const NEW_PASS_ENV: &str = "KS_NEW_PASSPHRASE_FILE";
const PASS_ENV: &str = "KS_PASSPHRASE_FILE";

struct T {
    dir: tempfile::TempDir,
    pass_file: PathBuf,
    vault: PathBuf,
}

fn mk() -> T {
    let dir = tempfile::tempdir().unwrap();
    let pass_file = dir.path().join("pass.txt");
    std::fs::write(&pass_file, PASS).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&pass_file, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    T {
        vault: dir.path().join("vault"),
        pass_file,
        dir,
    }
}

fn ks(t: &T, args: &[&str]) -> Command {
    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("ks"));
    c.arg("--vault")
        .arg(&t.vault)
        .args(args)
        .env(PASS_ENV, &t.pass_file)
        .env(NEW_PASS_ENV, &t.pass_file)
        .env_remove("KS_PASSPHRASE")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    c
}

fn run(t: &T, args: &[&str], stdin: &[u8]) -> (i32, String, String) {
    let mut c = ks(t, args).spawn().unwrap();
    c.stdin.take().unwrap().write_all(stdin).unwrap();
    let o = c.wait_with_output().unwrap();
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

fn json(t: &T, args: &[&str], stdin: &[u8]) -> (i32, serde_json::Value, String) {
    let mut a: Vec<&str> = args.into();
    a.push("--json");
    let (code, stdout, stderr) = run(t, &a, stdin);
    let v: serde_json::Value =
        serde_json::from_str(stdout.trim()).unwrap_or_else(|_| panic!("not json: {stdout}"));
    (code, v, stderr)
}

fn init(t: &T) {
    let (code, out, err) = run(t, &["init"], b"");
    assert!(code == 0, "init failed: {out} {err}");
    assert!(t.vault.join(".identity.age").exists());
    assert!(t.vault.join(".check.age").exists());
    assert!(t.vault.join(".gitignore").exists());
    assert!(t.vault.join(".gitattributes").exists());
}

#[test]
fn vault_init_creates_vault() {
    let t = mk();
    let pass = t.dir.path().join("new.txt");
    std::fs::write(&pass, PASS).unwrap();
    let mut c = ks(&t, &["init"]);
    c.env(NEW_PASS_ENV, &pass);
    c.env_remove(PASS_ENV);
    let o = c.spawn().unwrap().wait_with_output().unwrap();
    assert!(
        o.status.success(),
        "init failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    let (code, out, _) = run(&t, &["init"], b"");
    assert_eq!(code, 73, "non-empty dir must be exists: {out}");
}

#[test]
fn vault_ls_lists_sorted() {
    let t = mk();
    init(&t);
    run(&t, &["set", "b"], b"vb");
    run(&t, &["set", "a/x"], b"vx");
    let (_, v, err) = json(&t, &["ls"], b"");
    assert_eq!(v["entries"], serde_json::json!(["a/x", "b"]));
    assert_eq!(err, "");
    let (_, v, _) = json(&t, &["ls", "a"], b"");
    assert_eq!(v["entries"], serde_json::json!(["a/x"]));
}

#[test]
fn vault_show_and_field() {
    let t = mk();
    init(&t);
    run(&t, &["set", "gh/token", "-m"], b"tok123\nuser: alice\n");
    let (code, out, _) = run(&t, &["show", "gh/token"], b"");
    assert_eq!(code, 0);
    assert!(out.contains("tok123") && out.contains("user: alice"));
    let (_, v, _) = json(&t, &["show", "gh/token", "-f", "user"], b"");
    assert_eq!(v["value"], "alice");
    let (code, _, _) = run(&t, &["show", "nope"], b"");
    assert_eq!(code, 66);
    // binary entry: base64 in JSON
    run(&t, &["set", "bin", "-b"], &[0xde, 0xad]);
    let (_, v, _) = json(&t, &["show", "bin"], b"");
    assert_eq!(v["type"], "binary");
    assert_eq!(v["base64"], "3q0=");
    let (code, _, _) = run(&t, &["show", "bin", "-f", "x"], b"");
    assert_eq!(code, 64);
}

#[test]
fn vault_show_copy() {
    let t = mk();
    init(&t);
    run(&t, &["set", "a"], b"v");
    let (code, out, err) = run(&t, &["show", "a", "-c"], b"");
    assert!(
        code == 0 || err.contains("unavailable") || out.contains("unavailable"),
        "{code} {out} {err}"
    );
}

#[test]
fn vault_set_variants() {
    let t = mk();
    init(&t);
    let (code, _, _) = run(&t, &["set", "one"], b"single-line\nextra");
    assert_eq!(code, 0);
    let (_, out, _) = run(&t, &["show", "one"], b"");
    assert!(out.starts_with("single-line"));
    run(&t, &["set", "multi", "-m"], b"l1\r\nl2\nl3\n");
    let (_, out, _) = run(&t, &["show", "multi"], b"");
    assert!(out.contains("l1\nl2\nl3"));
    let (code, _, _) = run(&t, &["set", "empty"], b"\n");
    assert_eq!(code, 64);
}

#[test]
fn vault_overwrite_needs_force() {
    let t = mk();
    init(&t);
    run(&t, &["set", "a"], b"one");
    let (code, _, _) = run(&t, &["set", "a"], b"two");
    assert_eq!(code, 73);
    let (code, _, _) = run(&t, &["set", "a", "--force"], b"two");
    assert_eq!(code, 0);
    let (_, out, _) = run(&t, &["show", "a"], b"");
    assert_eq!(out.trim(), "two");
}

#[cfg(unix)]
#[test]
fn vault_edit_via_editor() {
    let t = mk();
    init(&t);
    run(&t, &["set", "doc"], b"line1");
    let script = t.dir.path().join("ed.sh");
    std::fs::write(&script, "#!/bin/sh\necho added >> \"$1\"\n").unwrap();
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let mut c = ks(&t, &["edit", "doc"]);
    c.env("EDITOR", script.to_str().unwrap());
    // interactive check needs TTY; edit requires interactive -> but stdin is piped => usage.
    let o = c.spawn().unwrap().wait_with_output().unwrap();
    assert_eq!(
        o.status.code(),
        Some(64),
        "non-interactive edit must be usage"
    );
}

#[test]
fn vault_gen() {
    let t = mk();
    init(&t);
    let (_, v, _) = json(&t, &["gen"], b"");
    assert_eq!(v["value"].as_str().unwrap().len(), 32);
    let (_, v, _) = json(&t, &["gen", "-l", "8", "-s", "hex"], b"");
    let s = v["value"].as_str().unwrap();
    assert_eq!(s.len(), 8);
    assert!(
        s.chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    );
    let (_, v, _) = json(&t, &["gen", "pw", "-l", "10"], b"");
    assert_eq!(v["stored"], true);
    let (_, out, _) = run(&t, &["show", "pw"], b"");
    assert_eq!(out.trim().len(), 10);
    let (code, _, _) = run(&t, &["gen", "pw", "--force"], b"");
    assert_eq!(code, 0);
    let (code, _, _) = json(&t, &["gen", "-l", "0"], b"");
    assert_eq!(code, 64);
}

#[test]
fn vault_rm_and_mv() {
    let t = mk();
    init(&t);
    run(&t, &["set", "x"], b"v");
    let (code, _, _) = run(&t, &["rm", "x"], b"");
    assert_eq!(code, 64, "rm needs --force non-interactively");
    let (code, _, _) = run(&t, &["rm", "x", "--force"], b"");
    assert_eq!(code, 0);
    let (code, _, _) = run(&t, &["show", "x"], b"");
    assert_eq!(code, 66);
    run(&t, &["set", "old"], b"v2");
    let (code, _, _) = run(&t, &["mv", "old", "new"], b"");
    assert_eq!(code, 0);
    let (_, out, _) = run(&t, &["show", "new"], b"");
    assert_eq!(out.trim(), "v2");
}

#[test]
fn vault_otp() {
    let t = mk();
    init(&t);
    // RFC 6238 SHA1 base32 "Hello!" secret
    run(
        &t,
        &["set", "totp"],
        b"otpauth://totp/x?secret=GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ",
    );
    let (_, v, _) = json(&t, &["otp", "totp"], b"");
    let code = v["code"].as_str().unwrap();
    assert!(
        code.len() == 6 && code.chars().all(|c| c.is_ascii_digit()),
        "{code}"
    );
    let valid_for = v["valid_for"].as_u64().unwrap();
    assert!((1..=30).contains(&valid_for));
}

#[test]
fn vault_run_injects_env() {
    let t = mk();
    init(&t);
    run(&t, &["set", "tok", "-m"], b"s3cret\nuser: u");
    let (code, out, _) = run(
        &t,
        &[
            "run",
            "-e",
            "MY_TOKEN=tok",
            "--",
            "sh",
            "-c",
            "printf %s $MY_TOKEN",
        ],
        b"",
    );
    assert_eq!(code, 0);
    assert_eq!(out, "s3cret");
    let (code, out, _) = run(
        &t,
        &["run", "-e", "U=tok#user", "--", "sh", "-c", "printf %s $U"],
        b"",
    );
    assert_eq!(out, "u");
    let (code, _, _) = run(
        &t,
        &["run", "-e", "9BAD=tok", "--", "sh", "-c", "true"],
        b"",
    );
    assert_eq!(code, 64);
    let (code, _, _) = run(&t, &["run", "-e", "E=tok", "--", "sh", "-c", "exit 7"], b"");
    assert_eq!(code, 7);
}

#[test]
fn vault_name_rules() {
    let t = mk();
    init(&t);
    let (code, _, err) = run(&t, &["set", "Bad Name"], b"v");
    assert_eq!(code, 64);
    assert!(err.contains("invalid") || err.contains("error"));
    let (code, _, _) = run(&t, &["set", "x.age"], b"v");
    assert_eq!(code, 64);
}

#[test]
fn vault_concurrent_processes() {
    let t = mk();
    init(&t);
    let mut handles = Vec::new();
    for p in 0..2 {
        let vault = t.vault.clone();
        let pf = t.pass_file.clone();
        handles.push(std::thread::spawn(move || {
            for i in 0..10 {
                let o = Command::new(assert_cmd::cargo::cargo_bin!("ks"))
                    .args(["set", &format!("p{p}/e{i}"), "--vault"])
                    .arg(&vault)
                    .env(PASS_ENV, &pf)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::null())
                    .stderr(Stdio::piped())
                    .spawn()
                    .and_then(|mut c| {
                        c.stdin.take().unwrap().write_all(b"v").unwrap();
                        c.wait_with_output()
                    })
                    .unwrap();
                assert!(
                    o.status.success(),
                    "{:?}",
                    String::from_utf8_lossy(&o.stderr)
                );
            }
        }));
    }
    for h in handles {
        h.join().unwrap();
    }
    let (_, v, _) = json(&t, &["ls"], b"");
    assert_eq!(v["entries"].as_array().unwrap().len(), 20);
}

#[test]
fn vault_forgery_and_hostile() {
    let t = mk();
    init(&t);
    run(&t, &["set", "a"], b"va");
    // swap a.age with a foreign-encrypted file is covered in the lib test;
    // here: rename two files and confirm corrupt.
    let root = &t.vault;
    run(&t, &["set", "b"], b"vb");
    let pa = root.join("a.age");
    let pb = root.join("b.age");
    let tmp = root.join(".tmpswap");
    std::fs::rename(&pa, &tmp).unwrap();
    std::fs::rename(&pb, &pa).unwrap();
    std::fs::rename(&tmp, &pb).unwrap();
    let (code, _, _) = run(&t, &["show", "a"], b"");
    assert_eq!(code, 65);
    // oversized entry
    let big = root.join("big.age");
    std::fs::File::create(&big)
        .unwrap()
        .set_len(70 * 1024 * 1024)
        .unwrap();
    let (code, _, _) = run(&t, &["show", "big"], b"");
    assert_eq!(code, 65);
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        symlink(&pa, root.join("ln.age")).unwrap();
        let (code, _, _) = run(&t, &["show", "ln"], b"");
        assert_eq!(code, 65);
    }
}

#[test]
fn unlock_passphrase_sources_and_policy() {
    let t = mk();
    init(&t);
    // wrong passphrase file
    let wrong = t.dir.path().join("wrong.txt");
    std::fs::write(&wrong, "wrong passphrase entirely").unwrap();
    let mut c = ks(&t, &["show", "x"]);
    c.env(PASS_ENV, &wrong);
    let o = c.spawn().unwrap().wait_with_output().unwrap();
    assert_eq!(o.status.code(), Some(77));
    // no source at all -> locked
    let mut c = ks(&t, &["show", "x"]);
    c.env_remove(PASS_ENV);
    let o = c.spawn().unwrap().wait_with_output().unwrap();
    assert_eq!(o.status.code(), Some(77));
    // weak new passphrase on init
    let weak = t.dir.path().join("weak.txt");
    std::fs::write(&weak, "short").unwrap();
    let t2 = mk();
    let mut c = ks(&t2, &["init"]);
    c.env(NEW_PASS_ENV, &weak);
    c.env_remove(PASS_ENV);
    let o = c.spawn().unwrap().wait_with_output().unwrap();
    assert_eq!(o.status.code(), Some(65));
}

#[test]
fn unlock_check_heal() {
    let t = mk();
    init(&t);
    std::fs::remove_file(t.vault.join(".check.age")).unwrap();
    let (_, v, _) = json(&t, &["ls"], b"");
    let (code, _, _) = run(&t, &["set", "x"], b"v");
    assert_eq!(code, 0, "unlock must heal a missing check file");
    assert!(t.vault.join(".check.age").exists());
    let (code, _, _) = run(&t, &["set", "y"], b"v");
    assert_eq!(code, 0);
}

#[test]
fn admin_passwd() {
    let t = mk();
    init(&t);
    let newf = t.dir.path().join("new.txt");
    std::fs::write(&newf, "a brand new memorable passphrase 9").unwrap();
    let mut c = ks(&t, &["passwd"]);
    c.env(NEW_PASS_ENV, &newf);
    let o = c.spawn().unwrap().wait_with_output().unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    // old passphrase no longer works
    let mut c = ks(&t, &["show", "x"]);
    let o = c.spawn().unwrap().wait_with_output().unwrap();
    assert_eq!(o.status.code(), Some(77));
    // new one works
    let newpass = t.dir.path().join("newpass.txt");
    std::fs::write(&newpass, "a brand new memorable passphrase 9").unwrap();
    let mut c = ks(&t, &["ls"]);
    c.env(PASS_ENV, &newpass);
    let o = c.spawn().unwrap().wait_with_output().unwrap();
    assert!(o.status.success());
}

#[test]
fn admin_doctor() {
    let t = mk();
    init(&t);
    let (_, v, _) = json(&t, &["doctor"], b"");
    assert_eq!(v["ok"], true);
    std::fs::write(t.vault.join(".tmpleft"), b"x").unwrap();
    std::fs::write(t.vault.join("notes.txt"), b"x").unwrap();
    let (_, v, _) = json(&t, &["doctor"], b"");
    assert_eq!(v["ok"], false);
    let probs = v["problems"].as_array().unwrap();
    assert!(probs.len() >= 2);
    let (code, _, _) = run(&t, &["doctor", "--fix"], b"");
    let (_, v, _) = json(&t, &["doctor"], b"");
    // .tmp fixed, notes.txt still reported
    assert!(
        v["problems"]
            .as_array()
            .unwrap()
            .iter()
            .all(|p| p["problem"] != "leftover temp file")
    );
}

#[test]
fn cli_json_contract() {
    let t = mk();
    init(&t);
    let (_, v, err) = json(&t, &["ls"], b"");
    assert_eq!(err, "");
    assert!(v.is_object());
    let (code, v, err) = json(&t, &["show", "missing"], b"");
    assert_eq!(code, 66);
    assert_eq!(v["error"]["code"], "not_found");
    assert_eq!(err, "");
}

#[test]
fn cli_errors_exit_codes() {
    let t = mk();
    init(&t);
    for (args, stdin, code) in [
        (vec!["set", "bad name"], &b"v"[..], 64),
        (vec!["show", "missing"], &b""[..], 66),
        (vec!["gen", "-l", "0"], &b""[..], 64),
    ] {
        let (c, _, _) = run(&t, &args, stdin);
        assert_eq!(c, code, "{args:?}");
    }
}

#[test]
fn cli_noninteractive_never_prompts() {
    let t = mk();
    init(&t);
    // no passphrase source, stdin closed: must fail fast, not prompt
    let mut c = ks(&t, &["show", "x"]);
    c.env_remove(PASS_ENV);
    let o = c.spawn().unwrap().wait_with_output().unwrap();
    assert_eq!(o.status.code(), Some(77));
}

#[test]
fn cli_help_no_hidden_commands() {
    let o = Command::new(assert_cmd::cargo::cargo_bin!("ks"))
        .arg("--help")
        .output()
        .unwrap();
    let help = String::from_utf8_lossy(&o.stdout);
    assert!(!help.contains("__clipboard"));
    // no example takes a passphrase as an argument
    assert!(!help.contains("KS_PASSPHRASE="));
}

#[cfg(unix)]
#[test]
fn cli_json_stderr_clean_and_broken_pipe() {
    let t = mk();
    init(&t);
    run(&t, &["set", "a"], b"v");
    // --json: stderr must be exactly empty
    let (code, _v, err) = json(&t, &["ls"], b"");
    assert_eq!(code, 0);
    assert_eq!(err, "");
    // warn path also stays silent in --json (group-readable pass file)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&t.pass_file, std::fs::Permissions::from_mode(0o644)).unwrap();
        let (_c, _v, err) = json(&t, &["ls"], b"");
        assert_eq!(err, "");
        std::fs::set_permissions(&t.pass_file, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    // broken pipe: `ks ls | head -c1` must not exit 101 (panic)
    let mut reader = Command::new(assert_cmd::cargo::cargo_bin!("ks"))
        .args(["ls", "--vault"])
        .arg(&t.vault)
        .env(PASS_ENV, &t.pass_file)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut stdout = reader.stdout.take().unwrap();
    use std::io::Read as _;
    let mut byte = [0u8; 1];
    let _ = stdout.read_exact(&mut byte);
    drop(stdout);
    let status = reader.wait().unwrap();
    assert_ne!(status.code(), Some(101), "ks panicked on a broken pipe");
}
