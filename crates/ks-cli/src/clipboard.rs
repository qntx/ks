//! Detached clipboard helper: `ks __clipboard <secs>` reads the value
//! from stdin, sets it (with platform privacy markers), reports `ok`,
//! and clears it after `secs` seconds if nothing else took the
//! clipboard. The parent waits for that `ok` before reporting success.

use std::io::{Read as _, Write as _};
use std::process::{Command, Stdio};

use zeroize::Zeroizing;

use ks::ErrorCode;

const DEFAULT_TIME: u64 = 45;
const ENV_TIME: &str = "KS_CLIP_TIME";

/// Seconds before the helper clears the clipboard.
fn clip_time() -> u64 {
    std::env::var(ENV_TIME)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_TIME)
}

/// Copies `value` to the clipboard via a detached helper so the plain
/// `ks` process can exit while the clear timer still runs.
pub(crate) fn copy(value: &[u8]) -> Result<(), (ErrorCode, String)> {
    let exe = std::env::current_exe().map_err(|e| {
        (
            ErrorCode::Unavailable,
            format!("cannot locate the ks binary: {e}"),
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

/// `__clipboard` helper entry point: exit codes only, the parent sees
/// `ok` on stdout once the value is actually on the clipboard. The
/// process lives until the timeout so the clear can run and (on X11/
/// Wayland) the selection keeps being served.
#[allow(
    clippy::disallowed_methods,
    reason = "the helper intentionally blocks until the clear deadline; there is no runtime"
)]
pub(crate) fn daemon(secs: u64) -> i32 {
    let mut value = Zeroizing::new(Vec::new());
    if std::io::stdin().read_to_end(&mut value).is_err() {
        return 1;
    }
    let Ok(utf8) = String::from_utf8(value.to_vec()) else {
        return 1;
    };
    let text = Zeroizing::new(utf8);
    let Ok(copied) = ks_clipboard::copy(&text) else {
        return 1;
    };
    drop(std::io::stdout().write_all(b"ok\n"));
    std::thread::sleep(std::time::Duration::from_secs(secs));
    ks_clipboard::clear_if_unchanged(&copied).ok();
    0
}

#[cfg(unix)]
fn detach(cmd: &mut Command) {
    use std::os::unix::process::CommandExt as _;
    cmd.process_group(0);
}

#[cfg(windows)]
fn detach(cmd: &mut Command) {
    use std::os::windows::process::CommandExt as _;
    cmd.creation_flags(0x0000_0200);
}
