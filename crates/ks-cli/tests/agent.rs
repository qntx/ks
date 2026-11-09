//! M3 session agent tests (real `ks __agent` processes).
//!
//! Isolation: every test vault gets a unique id, so endpoints never collide
//! with a real user's agents; every test locks its agent when done (the
//! `Guard` runs `ks lock` even on failure).
#![allow(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    clippy::string_lit_as_bytes,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::missing_assert_message,
    clippy::indexing_slicing,
    clippy::let_underscore_must_use,
    clippy::shadow_unrelated,
    clippy::items_after_statements,
    reason = "integration test harness: panicking on setup and terse assertions are intentional"
)]

use std::io::Write as _;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const PASS: &str = "correct horse battery staple fixture";

struct T {
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

/// Locks the agent on drop so tests never leave daemons behind.
struct Guard<'a>(&'a T);
impl Drop for Guard<'_> {
    fn drop(&mut self) {
        let _ = ks(self.0, &["lock"], b"", false);
    }
}

fn init(t: &T) {
    let (code, out, err) = ks(t, &["init"], b"", true);
    assert!(code == 0, "init failed: {out} {err}");
}

fn unlock(t: &T, extra: &[&str]) -> (i32, String, String) {
    let mut args = vec!["unlock"];
    args.extend_from_slice(extra);
    ks(t, &args, b"", true)
}

fn agent_running(t: &T) -> bool {
    let (_, out, _) = ks(t, &["status", "--json"], b"", false);
    serde_json::from_str::<serde_json::Value>(out.trim())
        .ok()
        .and_then(|v| v.get("agent").cloned())
        .is_some_and(|a| !a.is_null())
}

fn wait_gone(t: &T, deadline: Duration) -> bool {
    let end = Instant::now() + deadline;
    while Instant::now() < end {
        if !agent_running(t) {
            return true;
        }
        #[allow(
            clippy::disallowed_methods,
            reason = "polling loop in a test, no async runtime"
        )]
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

#[test]
fn agent_unlock_then_commands_without_passphrase() {
    let t = mk();
    init(&t);
    let _g = Guard(&t);
    let (code, out, err) = unlock(&t, &[]);
    assert!(code == 0, "unlock failed: {out} {err}");
    assert!(agent_running(&t));
    // modifying commands must not need a passphrase source any more
    for (args, input) in [
        (vec!["set", "tok"], "secret".as_bytes()),
        (vec!["show", "tok"], b""),
        (vec!["ls"], b""),
        (vec!["doctor"], b""),
    ] {
        let (code, out, err) = ks(&t, &args, input, false);
        assert!(
            code == 0,
            "{args:?} failed without agent passphrase: {out} {err}"
        );
    }
    // lock is idempotent
    let (code, out, _) = ks(&t, &["lock", "--json"], b"", false);
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert_eq!(v["locked"], true);
    assert_eq!(v["was_running"], true);
    assert!(!agent_running(&t));
    let (_, out, _) = ks(&t, &["lock", "--json"], b"", false);
    let v: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert_eq!(v["was_running"], false);
}

#[test]
fn agent_lock_stops_the_agent() {
    let t = mk();
    init(&t);
    let _g = Guard(&t);
    let (code, out, err) = unlock(&t, &[]);
    assert!(code == 0, "unlock failed: {out} {err}");
    assert!(agent_running(&t));
    let (code, _, _) = ks(&t, &["lock"], b"", false);
    assert_eq!(code, 0);
    assert!(wait_gone(&t, Duration::from_secs(5)));
    // without the agent, commands need a passphrase source again
    let (code, _, _) = ks(&t, &["ls"], b"", false);
    assert_eq!(code, 0);
    let (code, _, _) = ks(&t, &["show", "x"], b"", false);
    assert_eq!(code, 77);
}

#[test]
fn agent_ttl_expires() {
    let t = mk();
    init(&t);
    let _g = Guard(&t);
    let (code, _, err) = unlock(&t, &["--ttl", "1s", "--max", "10s"]);
    assert!(code == 0, "unlock failed: {err}");
    assert!(
        wait_gone(&t, Duration::from_secs(10)),
        "agent did not exit on ttl"
    );
    // without a passphrase source, commands are locked again
    let (code, _, _) = ks(&t, &["show", "x"], b"", false);
    assert_eq!(code, 77);
}

#[test]
fn agent_status_json_shape() {
    let t = mk();
    init(&t);
    let _g = Guard(&t);
    let (_, out, _) = ks(&t, &["status", "--json"], b"", false);
    let v: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert!(v["agent"].is_null());
    assert_eq!(v["git"]["repo"], false);
    unlock(&t, &[]);
    let (_, out, _) = ks(&t, &["status", "--json"], b"", false);
    let v: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    let agent = &v["agent"];
    assert!(agent["ttl_left"].as_u64().is_some_and(|s| s <= 900));
    assert!(agent["life_left"].as_u64().is_some_and(|s| s <= 28_800));
}

#[cfg(unix)]
#[test]
fn agent_impersonation_with_foreign_key() {
    // Vault A has a running __agent fed vault B's identity: unwrap works but
    // the check fails, so unlock falls back to the passphrase and asks the
    // impostor to exit.
    let a = mk();
    let b = mk();
    init(&a);
    init(&b);
    let _ga = Guard(&a);
    let _gb = Guard(&b);

    // extract B's identity text via the library
    let b_vault = ks::Vault::open(&b.vault).unwrap();
    let b_session = b_vault
        .unlock_without_agent(Some(&|| Ok(age::secrecy::SecretString::from(PASS))))
        .unwrap();
    let b_identity = b_session.agent_identity().unwrap();
    use age::secrecy::ExposeSecret as _;
    let b_text = b_identity.expose_secret().to_owned();

    // spawn `ks __agent` for A with B's identity
    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("ks"));
    c.arg("--vault")
        .arg(&a.vault)
        .args(["__agent", "--ttl", "60", "--max", "60"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut impostor = c.spawn().unwrap();
    impostor
        .stdin
        .take()
        .unwrap()
        .write_all(format!("{b_text}\n").as_bytes())
        .unwrap();
    // it should print ready after binding
    use std::io::{BufRead as _, BufReader};
    let mut ready = String::new();
    BufReader::new(impostor.stdout.take().unwrap())
        .read_line(&mut ready)
        .ok();
    assert!(ready.contains("ready"), "impostor did not start: {ready}");

    // commands with a passphrase source work AND warn about the stale agent
    let (code, _, err) = ks(&a, &["set", "x"], b"val", true);
    assert_eq!(code, 0, "{err}");
    assert!(
        err.contains("session agent"),
        "expected stale-agent warning: {err}"
    );
    // the impostor was asked to exit
    assert!(
        wait_gone(&a, Duration::from_secs(10)),
        "impostor kept running"
    );
    // the entry decrypts with A's real key
    let (_, out, _) = ks(&a, &["show", "x", "--no-agent"], b"", true);
    assert_eq!(out.trim(), "val");
    impostor.kill().ok();
    impostor.wait().ok();

    // with NO passphrase source and an impostor running -> locked, nothing written
    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("ks"));
    c.arg("--vault")
        .arg(&a.vault)
        .args(["__agent", "--ttl", "30", "--max", "30"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut impostor = c.spawn().unwrap();
    impostor
        .stdin
        .take()
        .unwrap()
        .write_all(format!("{b_text}\n").as_bytes())
        .unwrap();
    let (code, _, _) = ks(&a, &["set", "y"], b"v", false);
    assert_eq!(code, 77);
    let _ = ks(&a, &["lock"], b"", false);
    impostor.kill().ok();
    impostor.wait().ok();
}

#[test]
fn admin_passwd_skips_agent() {
    let t = mk();
    init(&t);
    let _g = Guard(&t);
    unlock(&t, &[]);
    // no passphrase source + agent running: passwd still needs the real passphrase
    let (code, _, _) = ks(&t, &["passwd"], b"", false);
    assert_eq!(code, 77);
    // with a source it works
    let newpass = t.dir.path().join("new.txt");
    std::fs::write(&newpass, "a brand new passphrase for the test").unwrap();
    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("ks"));
    c.arg("--vault")
        .arg(&t.vault)
        .arg("passwd")
        .env("KS_PASSPHRASE_FILE", &t.pass)
        .env("KS_NEW_PASSPHRASE_FILE", &newpass)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let o = c.output().unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
}

/// Windows: the agent starts and serves with the owner-only DACL applied
/// to its named pipe. Exercises the `owner_only_dacl` + `security_descriptor`
/// path; same-user enforcement then relies on that descriptor.
#[cfg(windows)]
#[test]
fn agent_windows_owner_dacl_roundtrip() {
    let t = mk();
    init(&t);
    let _g = Guard(&t);
    let (code, out, err) = unlock(&t, &[]);
    assert!(code == 0, "unlock failed: {out} {err}");
    let (code, out, err) = ks(&t, &["set", "tok"], b"v", false);
    assert!(code == 0, "set via agent failed: {out} {err}");
    let (code, out, _) = ks(&t, &["show", "tok"], b"", false);
    assert_eq!(code, 0);
    assert_eq!(out.trim(), "v");
}

/// A server started with `Host::Desktop` in-process: `ks unlock` must
/// refuse to replace it, and `ks status` reports the host.
#[test]
fn unlock_refuses_to_replace_desktop_host() {
    let t = mk();
    init(&t);
    let pass = PASS.to_owned();
    let v = ks::Vault::open(&t.vault).unwrap();
    let session = v
        .unlock_without_agent(Some(&|| Ok(age::secrecy::SecretString::from(pass.clone()))))
        .unwrap();
    let identity = session.agent_identity().unwrap();
    let server = ks::agent::Server::start(
        &v,
        identity,
        ks::agent::ServerConfig {
            ttl: Duration::from_secs(30),
            max: Duration::from_secs(300),
            host: ks::agent::Host::Desktop,
        },
    )
    .unwrap();

    let (code, _out, err) = unlock(&t, &[]);
    assert_eq!(code, 69, "desktop host must refuse unlock: {err}");
    assert!(
        err.contains("desktop"),
        "message should name the desktop host: {err}"
    );

    let (_, out, _) = ks(&t, &["status", "--json"], b"", false);
    let j: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert_eq!(j["agent"]["host"], "desktop");

    let _ = server.stop();
}
