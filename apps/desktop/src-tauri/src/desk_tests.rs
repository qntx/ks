//! DESK-CORE command behavior tests (D1 exit criterion). Commands are
//! invoked directly with `tauri::State` borrowed from a mock runtime
//! app; vaults are real, in tempdirs.
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::tests_outside_test_module,
    clippy::missing_assert_message,
    clippy::allow_attributes,
    clippy::excessive_nesting,
    clippy::items_after_statements,
    clippy::shadow_unrelated,
    clippy::str_to_string,
    clippy::redundant_pub_crate,
    clippy::uninlined_format_args,
    clippy::needless_raw_strings,
    clippy::let_underscore_must_use,
    clippy::disallowed_methods,
    clippy::clone_on_ref_ptr,
    clippy::redundant_clone,
    clippy::significant_drop_tightening,
    clippy::many_single_char_names,
    reason = "test harness: panicking asserts and verbose fixtures are intentional"
)]

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use age::secrecy::SecretString;
use sha2::Digest as _;
use tauri::Manager as _;
use tempfile::TempDir;

use crate::commands;
use crate::secret::SecretText;
use crate::state::AppState;

const PASS: &str = "correct horse battery staple fixture";

struct Fixture {
    _dir: TempDir,
    vault: PathBuf,
    app: tauri::App<tauri::test::MockRuntime>,
    state: Arc<AppState>,
}

fn mk() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let vault = dir.path().join("v");
    let _s = ks::Vault::create(&vault, &SecretString::from(PASS)).unwrap();
    let app = tauri::test::mock_app();
    let state = Arc::new(AppState::new());
    app.manage(state.clone());
    let f = Fixture {
        _dir: dir,
        vault,
        app,
        state,
    };
    commands::open_vault(
        f.app.state::<Arc<AppState>>(),
        f.vault.display().to_string(),
    )
    .unwrap();
    f
}

fn mk_git() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let vault = dir.path().join("v");
    let app = tauri::test::mock_app();
    let state = Arc::new(AppState::new());
    app.manage(state.clone());
    commands::create_vault(
        app.handle().clone(),
        app.state::<Arc<AppState>>(),
        vault.display().to_string(),
        SecretText::new(PASS),
        true,
    )
    .unwrap();
    let f = Fixture {
        _dir: dir,
        vault,
        app,
        state,
    };
    git_local_config(&f.vault);
    f
}

/// Unlocks through the same `Vault::unlock_without_agent` path the
/// `unlock` command uses, storing the session in `AppState`. The
/// command fn itself needs a `AppHandle<Wry>` (for watcher + App Nap
/// dispatch), which the mock runtime cannot provide — the error
/// mapping and session effects are what this suite asserts.
fn unlock(f: &Fixture) -> (Option<crate::error::AppError>, Vec<String>) {
    let pass = SecretString::from(PASS);
    let mut i = f.state.inner.lock().unwrap();
    let o = i.open.as_mut().unwrap();
    match o.vault.unlock_without_agent(Some(&|| Ok(pass.clone()))) {
        Ok(s) => {
            let w = s.warnings().to_vec();
            o.session = Some(s);
            o.unlocked_at = Some(std::time::Instant::now());
            o.last_activity = std::time::Instant::now();
            (None, w)
        }
        Err(e) => (Some(e.into()), Vec::new()),
    }
}

fn code<T>(r: Result<T, crate::error::AppError>) -> String {
    r.err().map_or(String::new(), |e| e.code)
}

fn set(f: &Fixture, name: &str, text: &str, overwrite: bool) {
    commands::save_entry(
        f.app.state::<Arc<AppState>>(),
        name.to_owned(),
        SecretText::new(text),
        overwrite,
    )
    .unwrap();
}

#[test]
fn desk_core_unlock_wrong_passphrase_then_success() {
    let f = mk();
    let vault = f.vault.clone();
    let v = ks::Vault::open(&vault).unwrap();
    let err = v
        .unlock_without_agent(Some(&|| {
            Ok(SecretString::from("wrong wrong wrong wrong wrong"))
        }))
        .unwrap_err();
    let err: crate::error::AppError = err.into();
    assert_eq!(err.code, "wrong_passphrase");
    assert!(!commands::app_state(f.app.state::<Arc<AppState>>()).unlocked);
    unlock(&f);
    assert!(commands::app_state(f.app.state::<Arc<AppState>>()).unlocked);
}

#[test]
fn desk_core_every_vault_command_is_locked_before_unlock() {
    let f = mk();
    let st = || f.app.state::<Arc<AppState>>();
    let cases: Vec<(&str, String)> = vec![
        ("list_entries", code(commands::list_entries(st()))),
        (
            "entry_detail",
            code(commands::entry_detail(st(), "a".into())),
        ),
        (
            "reveal_field",
            code(commands::reveal_field(
                st(),
                "a".into(),
                commands::FieldRef::Value,
            )),
        ),
        (
            "copy_field",
            code(commands::copy_field(
                st(),
                "a".into(),
                commands::FieldRef::Value,
            )),
        ),
        ("totp_code", code(commands::totp_code(st(), "a".into()))),
        ("edit_entry", code(commands::edit_entry(st(), "a".into()))),
        (
            "save_entry",
            code(commands::save_entry(
                st(),
                "a".into(),
                SecretText::new("v"),
                false,
            )),
        ),
        (
            "rename_entry",
            code(commands::rename_entry(st(), "a".into(), "b".into(), false)),
        ),
        (
            "delete_entry",
            code(commands::delete_entry(st(), "a".into())),
        ),
    ];
    for (name, code) in cases {
        assert_eq!(code, "locked", "{name} must be locked");
    }
}

#[test]
fn desk_core_reveal_detail_has_lengths_only_and_reveal_returns_value() {
    let f = mk();
    unlock(&f);
    set(&f, "tok", "secret\nfield: value123\nnotes: hi", false);
    let d = commands::entry_detail(f.app.state::<Arc<AppState>>(), "tok".into()).unwrap();
    assert_eq!(d.kind, "text");
    assert!(d.value.is_some());
    assert_eq!(d.value.unwrap().length, 6);
    assert_eq!(d.fields.len(), 1);
    assert_eq!(d.fields[0].key, "field");
    assert_eq!(d.fields[0].length, 8);
    assert_eq!(d.notes_length, 2);
    assert!(!d.has_totp);
    assert_eq!(
        commands::reveal_field(
            f.app.state::<Arc<AppState>>(),
            "tok".into(),
            commands::FieldRef::Value,
        )
        .unwrap()
        .expose(),
        "secret"
    );
    assert_eq!(
        commands::reveal_field(
            f.app.state::<Arc<AppState>>(),
            "tok".into(),
            commands::FieldRef::Field("field".into()),
        )
        .unwrap()
        .expose(),
        "value123"
    );
    assert_eq!(
        commands::reveal_field(
            f.app.state::<Arc<AppState>>(),
            "tok".into(),
            commands::FieldRef::Notes,
        )
        .unwrap()
        .expose(),
        "hi"
    );
}

#[test]
fn desk_core_totp_code_generated_in_rust() {
    let f = mk();
    unlock(&f);
    set(
        &f,
        "otp",
        "value\ntotp: otpauth://totp/x?secret=GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ",
        false,
    );
    let t = commands::totp_code(f.app.state::<Arc<AppState>>(), "otp".into()).unwrap();
    assert_eq!(t.code.len(), 6);
    assert!(t.code.chars().all(|c| c.is_ascii_digit()));
    assert!(t.valid_for <= 30, "valid_for={}", t.valid_for);
    let err = commands::totp_code(f.app.state::<Arc<AppState>>(), "none".into()).unwrap_err();
    assert_eq!(err.code, "not_found");
}

#[test]
fn desk_core_edit_save_rename_delete_overwrite_rules() {
    let f = mk();
    unlock(&f);
    set(&f, "a", "one", false);
    let err = commands::save_entry(
        f.app.state::<Arc<AppState>>(),
        "a".into(),
        SecretText::new("two"),
        false,
    )
    .unwrap_err();
    assert_eq!(err.code, "exists");
    set(&f, "a", "two", true);
    assert_eq!(
        commands::edit_entry(f.app.state::<Arc<AppState>>(), "a".into())
            .unwrap()
            .expose(),
        "two"
    );
    commands::rename_entry(
        f.app.state::<Arc<AppState>>(),
        "a".into(),
        "b".into(),
        false,
    )
    .unwrap();
    assert_eq!(
        commands::list_entries(f.app.state::<Arc<AppState>>()).unwrap(),
        vec!["b".to_owned()]
    );
    commands::delete_entry(f.app.state::<Arc<AppState>>(), "b".into()).unwrap();
    assert_eq!(
        commands::list_entries(f.app.state::<Arc<AppState>>()).unwrap(),
        Vec::<String>::new()
    );
}

#[test]
fn desk_core_edit_git_autocommits_on_writes() {
    let f = mk_git();
    set(&f, "a", "one", false);
    commands::rename_entry(
        f.app.state::<Arc<AppState>>(),
        "a".into(),
        "b".into(),
        false,
    )
    .unwrap();
    commands::delete_entry(f.app.state::<Arc<AppState>>(), "b".into()).unwrap();
    let out = std::process::Command::new("git")
        .args(["-C"])
        .arg(&f.vault)
        .args(["log", "--format=%s"])
        .output()
        .unwrap();
    let log = String::from_utf8_lossy(&out.stdout);
    assert!(log.contains("set a"), "{log}");
    assert!(log.contains("mv a b"), "{log}");
    assert!(log.contains("rm b"), "{log}");
}

#[cfg(target_os = "macos")]
#[test]
fn desk_core_copy_holds_then_clears_clipboard() {
    // save the user's clipboard string
    let prev = std::process::Command::new("pbpaste").output().unwrap();
    let f = mk();
    unlock(&f);
    set(&f, "a", "ks-clip-value", false);
    commands::set_settings(crate::settings::Settings {
        clipboard_seconds: 1,
        ..crate::settings::Settings::default()
    })
    .unwrap();
    let c = commands::copy_field(
        f.app.state::<Arc<AppState>>(),
        "a".into(),
        commands::FieldRef::Value,
    )
    .unwrap();
    assert!(c.clears_in_secs >= 1);
    let now = std::process::Command::new("pbpaste").output().unwrap();
    assert_eq!(now.stdout, b"ks-clip-value");
    for _ in 0..40 {
        std::thread::sleep(Duration::from_millis(100));
        let cur = std::process::Command::new("pbpaste").output().unwrap();
        if cur.stdout != b"ks-clip-value" {
            break;
        }
    }
    let cur = std::process::Command::new("pbpaste").output().unwrap();
    assert_ne!(cur.stdout, b"ks-clip-value");
    drop(
        std::process::Command::new("pbcopy")
            .stdin(std::process::Stdio::piped())
            .spawn()
            .map(|mut p| {
                use std::io::Write as _;
                let _ = p.stdin.take().unwrap().write_all(&prev.stdout);
                let _ = p.wait();
            }),
    );
}

#[test]
fn desk_core_autolock_manual_then_commands_locked() {
    let f = mk();
    unlock(&f);
    commands::lock(f.app.state::<Arc<AppState>>()).unwrap();
    assert!(!commands::app_state(f.app.state::<Arc<AppState>>()).unlocked);
    let err = commands::list_entries(f.app.state::<Arc<AppState>>()).unwrap_err();
    assert_eq!(err.code, "locked");
}

#[test]
fn desk_core_autolock_idle_timer_locks() {
    let f = mk();
    unlock(&f);
    let state = f.state.clone();
    let _w = crate::lock::start(
        state.clone(),
        Duration::from_millis(800),
        Duration::from_secs(3600),
    );
    for _ in 0..50 {
        std::thread::sleep(Duration::from_millis(100));
        if !commands::app_state(f.app.state::<Arc<AppState>>()).unlocked {
            break;
        }
    }
    assert!(!commands::app_state(f.app.state::<Arc<AppState>>()).unlocked);
}

#[test]
fn desk_core_watch_identity_change_locks_key_changed() {
    let f = mk();
    unlock(&f);
    // the `unlock` command registers this watcher; the test helper
    // bypasses it, so wire the same one here.
    {
        let mut i = f.state.inner.lock().unwrap();
        let o = i.open.as_mut().unwrap();
        let hash =
            sha2::Sha256::digest(std::fs::read(f.vault.join(".identity.age")).unwrap()).to_vec();
        o.watcher = crate::watch::Watcher::start(
            f.app.handle().clone(),
            f.state.clone(),
            f.vault.clone(),
            hash,
        );
    }
    let id = f.vault.join(".identity.age");
    let orig = std::fs::read(&id).unwrap();
    std::fs::write(&id, b"corrupted-identity").unwrap();
    for _ in 0..60 {
        std::thread::sleep(Duration::from_millis(100));
        if !commands::app_state(f.app.state::<Arc<AppState>>()).unlocked {
            break;
        }
    }
    assert!(
        !commands::app_state(f.app.state::<Arc<AppState>>()).unlocked,
        "identity change must lock the session"
    );
    std::fs::write(&id, orig).ok();
}

#[test]
fn desk_core_onboard_create_open_invalid_and_settings() {
    let f = mk();
    let err = commands::open_vault(
        f.app.state::<Arc<AppState>>(),
        "/nonexistent/vault".to_owned(),
    )
    .unwrap_err();
    assert_ne!(err.code, "");

    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("new-vault");
    let v = commands::create_vault(
        f.app.handle().clone(),
        f.app.state::<Arc<AppState>>(),
        p.display().to_string(),
        SecretText::new(PASS),
        false,
    )
    .unwrap();
    assert_eq!(v.path, p.canonicalize().unwrap().display().to_string());
    assert!(p.join(".identity.age").exists());
    // create keeps the session: the new vault is already unlocked.
    assert!(commands::app_state(f.app.state::<Arc<AppState>>()).unlocked);
    assert_eq!(
        commands::list_entries(f.app.state::<Arc<AppState>>()).unwrap(),
        Vec::<String>::new()
    );

    let mut s = commands::get_settings();
    s.auto_lock_minutes = 7;
    commands::set_settings(s.clone()).unwrap();
    let loaded = commands::get_settings();
    assert_eq!(loaded.auto_lock_minutes, 7);
}

#[test]
fn desk_core_onboard_clone_local_repo() {
    let f = mk();
    unlock(&f);
    set(&f, "a", "v", false);
    // source vault must be a git repo with a repo-local identity
    let st = std::process::Command::new("git")
        .args(["-C"])
        .arg(&f.vault)
        .args(["init"])
        .status()
        .unwrap();
    assert!(st.success());
    git_local_config(&f.vault);
    let st = std::process::Command::new("git")
        .args(["-C"])
        .arg(&f.vault)
        .args(["add", "-Af", "--", ".identity.age", "*.age"])
        .status()
        .unwrap();
    assert!(st.success());
    let st = std::process::Command::new("git")
        .args(["-C"])
        .arg(&f.vault)
        .args(["commit", "-qm", "src"])
        .status()
        .unwrap();
    assert!(st.success());
    let dir = tempfile::tempdir().unwrap();
    let dst = dir.path().join("cloned");
    let v = tauri::async_runtime::block_on(commands::clone_vault(
        f.app.handle().clone(),
        f.app.state::<Arc<AppState>>(),
        f.vault.display().to_string(),
        dst.display().to_string(),
        SecretText::new(PASS),
    ))
    .unwrap();
    assert_eq!(v.path, dst.canonicalize().unwrap().display().to_string());
    assert!(dst.join(".identity.age").exists());
    // clone keeps the session: the cloned vault is already unlocked.
    assert_eq!(
        commands::list_entries(f.app.state::<Arc<AppState>>()).unwrap(),
        vec!["a".to_owned()]
    );
}

#[test]
fn desk_core_totp_field_ref_returns_code_not_seed() {
    let f = mk();
    unlock(&f);
    set(
        &f,
        "otp",
        "value\ntotp: GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ",
        false,
    );
    let code = commands::reveal_field(
        f.app.state::<Arc<AppState>>(),
        "otp".into(),
        commands::FieldRef::Totp,
    )
    .unwrap();
    assert_eq!(code.expose().len(), 6);
    assert!(code.expose().chars().all(|c| c.is_ascii_digit()));
    assert_ne!(code.expose(), "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ");
    #[cfg(target_os = "macos")]
    {
        let c = commands::copy_field(
            f.app.state::<Arc<AppState>>(),
            "otp".into(),
            commands::FieldRef::Totp,
        )
        .unwrap();
        assert!(c.clears_in_secs > 0);
    }
}

#[test]
fn desk_core_last_vault_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("v");
    // Other tests share this process's state file; retry to win any race.
    for _ in 0..50 {
        crate::settings::remember_vault(&p);
        if crate::settings::last_vault().as_deref() == Some(p.as_path()) {
            return;
        }
    }
    panic!("last_vault did not round-trip");
}

/// Repo-local git identity so tests never read the developer's or
/// runner's global git config (no signing hooks either).
fn git_local_config(vault: &PathBuf) {
    for args in [
        vec!["config", "user.name", "ks-test"],
        vec!["config", "user.email", "ks-test@local"],
        vec!["config", "commit.gpgsign", "false"],
    ] {
        let st = std::process::Command::new("git")
            .args(["-C"])
            .arg(vault)
            .args(&args)
            .status()
            .unwrap();
        assert!(st.success());
    }
}

#[test]
fn desk_core_watchdog_thread_exits_after_auto_lock() {
    let f = mk();
    let (err, _) = unlock(&f);
    assert!(err.is_none());
    // idle=ZERO: the first tick's deadline is already past.
    let tx = crate::lock::start(f.state.clone(), Duration::ZERO, Duration::from_secs(3600));
    for _ in 0..200 {
        if !commands::app_state(f.app.state::<Arc<AppState>>()).unlocked {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(!commands::app_state(f.app.state::<Arc<AppState>>()).unlocked);
    std::thread::sleep(Duration::from_secs(1));
    // The receiver is gone: the watchdog returned instead of busy-spinning.
    assert!(tx.send(crate::lock::Wake).is_err());
}

/// Real unlock through the command so `begin_session` hosts the agent.
fn unlock_via_command(f: &Fixture) -> Vec<String> {
    commands::unlock(
        f.app.handle().clone(),
        f.app.state::<Arc<AppState>>(),
        Some(SecretText::new(PASS)),
    )
    .unwrap()
    .warnings
}

fn poll_locked(f: &Fixture, max_ms: u64) -> bool {
    for _ in 0..(max_ms / 50) {
        if !commands::app_state(f.app.state::<Arc<AppState>>()).unlocked {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    !commands::app_state(f.app.state::<Arc<AppState>>()).unlocked
}

fn last_reason(f: &Fixture) -> Option<crate::lock::Reason> {
    f.state.inner.lock().unwrap().last_lock_reason
}

#[test]
fn desk_core_agent_desktop_host_and_terminal_read() {
    let f = mk();
    let warnings = unlock_via_command(&f);
    assert!(
        !warnings
            .iter()
            .any(|w| w.contains("terminal session unavailable")),
        "agent should host: {warnings:?}"
    );
    let v = ks::Vault::open(&f.vault).unwrap();
    let st = ks::agent::status(&v).unwrap().expect("agent should run");
    assert_eq!(st.host, "desktop");

    set(&f, "work/gh", "s3cr3t-value\nuser: octo", false);
    let s2 = v.unlock(None).unwrap();
    let e = s2.get(&ks::Name::parse("work/gh").unwrap()).unwrap();
    assert_eq!(e.as_text().unwrap().value(), "s3cr3t-value");

    commands::lock(f.app.state::<Arc<AppState>>()).unwrap();
    assert!(poll_locked(&f, 2000));
}

#[test]
fn desk_core_agent_manual_lock_stops_endpoint() {
    let f = mk();
    let _ = unlock_via_command(&f);
    let v = ks::Vault::open(&f.vault).unwrap();
    assert!(ks::agent::status(&v).unwrap().is_some());
    commands::lock(f.app.state::<Arc<AppState>>()).unwrap();
    // During teardown the socket may still exist while handshakes fail;
    // only Ok(None) proves the endpoint is gone.
    let mut gone = false;
    for _ in 0..80 {
        if matches!(ks::agent::status(&v), Ok(None)) {
            gone = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(gone, "agent endpoint should be gone after lock");
}

#[test]
fn desk_core_agent_terminal_lock_locks_app() {
    let f = mk();
    let _ = unlock_via_command(&f);
    let v = ks::Vault::open(&f.vault).unwrap();
    assert!(ks::agent::lock(&v).unwrap());
    assert!(
        poll_locked(&f, 3000),
        "terminal ks lock should lock the app"
    );
    assert_eq!(last_reason(&f).unwrap(), crate::lock::Reason::Terminal);
}

#[test]
fn desk_core_agent_takeover_asks_cli_to_exit() {
    let f = mk();
    // A `ks unlock`-style CLI agent is already serving.
    let v = ks::Vault::open(&f.vault).unwrap();
    let s = v
        .unlock_without_agent(Some(&|| Ok(SecretString::from(PASS))))
        .unwrap();
    let cli = ks::agent::Server::start(
        &v,
        s.agent_identity().unwrap(),
        ks::agent::ServerConfig {
            ttl: Duration::from_secs(300),
            max: Duration::from_secs(3600),
            host: ks::agent::Host::Cli,
        },
    )
    .unwrap();
    drop(s);
    let cli_wait = std::thread::spawn(move || cli.wait());

    let warnings = unlock_via_command(&f);
    assert!(
        !warnings
            .iter()
            .any(|w| w.contains("terminal session unavailable")),
        "takeover should host: {warnings:?}"
    );
    let st = ks::agent::status(&v).unwrap().expect("agent should run");
    assert_eq!(st.host, "desktop");
    assert_eq!(
        cli_wait.join().unwrap(),
        ks::agent::StopReason::LockRequested
    );

    commands::lock(f.app.state::<Arc<AppState>>()).unwrap();
}

#[test]
fn desk_core_agent_terminal_activity_counts_toward_idle() {
    let f = mk();
    let _ = unlock_via_command(&f);
    set(&f, "k", "v", false);
    // Shrink the app watchdog to a test-sized idle window.
    {
        let mut i = f.state.inner.lock().unwrap();
        let o = i.open.as_mut().unwrap();
        o.start_watchdog(
            f.state.clone(),
            Duration::from_secs(2),
            Duration::from_secs(3600),
        );
    }
    let v = ks::Vault::open(&f.vault).unwrap();
    // ~3s of terminal traffic every 500ms, no app heartbeats: stays unlocked.
    for _ in 0..6 {
        let s2 = v.unlock(None).unwrap();
        let _e = s2.get(&ks::Name::parse("k").unwrap()).unwrap();
        drop(s2);
        std::thread::sleep(Duration::from_millis(500));
        assert!(commands::app_state(f.app.state::<Arc<AppState>>()).unlocked);
    }
    // Traffic stops: the idle watchdog fires within a few seconds.
    assert!(
        poll_locked(&f, 6000),
        "idle should lock after traffic stops"
    );
    assert_eq!(last_reason(&f).unwrap(), crate::lock::Reason::Idle);
}

#[test]
fn desk_core_agent_lock_between_install_and_start_leaves_no_agent() {
    let f = mk();
    // A `ks unlock`-style CLI agent makes `begin_session`'s takeover wait,
    // widening the install→agent-start window deterministically.
    let v = ks::Vault::open(&f.vault).unwrap();
    let s = v
        .unlock_without_agent(Some(&|| Ok(SecretString::from(PASS))))
        .unwrap();
    let cli = ks::agent::Server::start(
        &v,
        s.agent_identity().unwrap(),
        ks::agent::ServerConfig {
            ttl: Duration::from_secs(300),
            max: Duration::from_secs(3600),
            host: ks::agent::Host::Cli,
        },
    )
    .unwrap();
    drop(s);
    let cli_wait = std::thread::spawn(move || cli.wait());

    // Lock the moment the session is installed, while agent setup runs.
    let st2 = f.state.clone();
    let locker = std::thread::spawn(move || {
        for _ in 0..400 {
            let installed = st2
                .inner
                .lock()
                .unwrap()
                .open
                .as_ref()
                .is_some_and(|o| o.session.is_some());
            if installed {
                let _ = st2.lock(crate::lock::Reason::Manual);
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    });
    let _ = unlock_via_command(&f);
    locker.join().unwrap();
    assert_eq!(
        cli_wait.join().unwrap(),
        ks::agent::StopReason::LockRequested
    );
    // Either the generation check stopped the just-started server or the
    // lock did — the endpoint is gone and the app is locked.
    assert!(poll_locked(&f, 2000));
    for _ in 0..80 {
        if matches!(ks::agent::status(&v), Ok(None)) {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(matches!(ks::agent::status(&v), Ok(None)));
}
