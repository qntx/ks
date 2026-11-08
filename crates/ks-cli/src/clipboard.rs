//! Clipboard via a detached child: `ks __clipboard <secs>` reads the value
//! from stdin (never argv/env), sets the clipboard, and clears it after the
//! timeout only if the value is unchanged.

use std::io::{Read as _, Write as _};
use std::process::{Command, Stdio};

use ks::ErrorCode;
use zeroize::Zeroizing;

use crate::output::CliResult;

/// Seconds to keep the clipboard (`KS_CLIP_TIME`, default 45).
#[must_use]
pub(crate) fn clip_time() -> u64 {
    std::env::var("KS_CLIP_TIME")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(45)
}

/// Spawns the detached `ks __clipboard` helper and feeds it `value`.
///
/// # Errors
///
/// `unavailable` when the helper cannot be spawned or fed.
pub(crate) fn copy(value: &[u8]) -> CliResult<()> {
    let exe = std::env::current_exe().map_err(|e| {
        (
            ErrorCode::Unavailable,
            format!("cannot locate ks binary: {e}"),
        )
    })?;
    let mut cmd = Command::new(exe);
    cmd.args(["__clipboard", &clip_time().to_string()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    detach(&mut cmd);
    crate::harden::no_inherit_std_handles();
    let mut child = cmd.spawn().map_err(|e| {
        (
            ErrorCode::Unavailable,
            format!("cannot spawn clipboard helper: {e}"),
        )
    })?;
    if let Some(mut stdin) = child.stdin.take() {
        drop(stdin.write_all(value));
    }
    // The helper prints `ok` once the clipboard actually holds the value;
    // anything else (spawn-level clap errors included) means failure.
    let waited = wait_ok(child.stdout.take());
    if !waited {
        return Err((
            ErrorCode::Unavailable,
            "could not set the clipboard".to_owned(),
        ));
    }
    Ok(())
}

/// Waits up to 3 s for the helper's `ok` line.
fn wait_ok(stdout: Option<impl std::io::Read + Send + 'static>) -> bool {
    let Some(mut out) = stdout else {
        return false;
    };
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = [0u8; 8];
        let ok = out
            .read(&mut buf)
            .is_ok_and(|n| buf.get(..n) == Some(b"ok\n".as_slice()));
        tx.send(ok).ok();
    });
    rx.recv_timeout(std::time::Duration::from_secs(3))
        .unwrap_or(false)
}

/// Sets the clipboard in this process, waits `secs`, clears if unchanged.
/// Used by the `__clipboard` daemon subcommand.
#[allow(
    clippy::disallowed_methods,
    reason = "the daemon intentionally blocks for the TTL; there is no event loop to yield to"
)]
pub(crate) fn daemon(secs: u64) -> i32 {
    let mut value = Zeroizing::new(Vec::new());
    if std::io::stdin().read_to_end(&mut value).is_err() {
        return 1;
    }
    let Ok(mut cb) = arboard::Clipboard::new() else {
        return 1;
    };
    let Ok(utf8) = String::from_utf8(value.to_vec()) else {
        return 1;
    };
    let text = Zeroizing::new(utf8);
    if set(&mut cb, &text, secs).is_err() {
        return 1;
    }
    drop(std::io::stdout().write_all(b"ok\n"));
    // wait_until on linux already blocked for `secs`; other platforms sleep.
    #[cfg(not(target_os = "linux"))]
    std::thread::sleep(std::time::Duration::from_secs(secs));
    // Clear only if the clipboard still holds our value.
    if cb.get_text().ok().as_deref() == Some(text.as_str()) {
        cb.set_text(String::new()).ok();
    }
    0
}

#[cfg(target_os = "linux")]
fn set(cb: &mut arboard::Clipboard, text: &str, secs: u64) -> Result<(), arboard::Error> {
    // wait_until keeps ownership until the deadline, then returns; the clear
    // below still runs at `secs`, not an hour later.
    use arboard::SetExtLinux as _;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(secs);
    cb.set().wait_until(deadline).text(text)
}

#[cfg(not(target_os = "linux"))]
fn set(cb: &mut arboard::Clipboard, text: &str, _secs: u64) -> Result<(), arboard::Error> {
    cb.set_text(text)
}

#[cfg(unix)]
fn detach(cmd: &mut Command) {
    use std::os::unix::process::CommandExt as _;
    cmd.process_group(0);
}

#[cfg(windows)]
fn detach(cmd: &mut Command) {
    use std::os::windows::process::CommandExt as _;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(DETACHED_PROCESS | CREATE_NO_WINDOW);
}

#[cfg(not(any(unix, windows)))]
fn detach(_cmd: &mut Command) {}
