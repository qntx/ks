//! Clipboard via a detached child: `ks __clipboard <secs>` reads the value
//! from stdin (never argv/env), sets the clipboard, and clears it after the
//! timeout only if the value is unchanged.

use std::io::Write as _;
use std::process::{Command, Stdio};

use ks::ErrorCode;

use crate::output::CliResult;

/// Seconds to keep the clipboard (`KS_CLIP_TIME`, default 45).
#[must_use]
pub fn clip_time() -> u64 {
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
pub fn copy(value: &[u8]) -> CliResult<()> {
    let exe = std::env::current_exe().map_err(|e| {
        (
            ErrorCode::Unavailable,
            format!("cannot locate ks binary: {e}"),
        )
    })?;
    let mut cmd = Command::new(exe);
    cmd.args(["__clipboard", &clip_time().to_string()])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    detach(&mut cmd);
    let mut child = cmd.spawn().map_err(|e| {
        (
            ErrorCode::Unavailable,
            format!("cannot spawn clipboard helper: {e}"),
        )
    })?;
    if let Some(mut stdin) = child.stdin.take() {
        drop(stdin.write_all(value));
    }
    Ok(())
}

/// Sets the clipboard in this process, waits `secs`, clears if unchanged.
/// Used by the `__clipboard` daemon subcommand.
pub fn daemon(secs: u64) -> i32 {
    use std::io::Read as _;
    let mut value = Vec::new();
    if std::io::stdin().read_to_end(&mut value).is_err() {
        return 1;
    }
    let Ok(mut cb) = arboard::Clipboard::new() else {
        return 1;
    };
    let Ok(text) = String::from_utf8(value) else {
        return 1;
    };
    if set(&mut cb, &text).is_err() {
        return 1;
    }
    std::thread::sleep(std::time::Duration::from_secs(secs));
    // Clear only if the clipboard still holds our value.
    if cb.get_text().ok().as_deref() == Some(text.as_str()) {
        let _ = cb.set_text(String::new());
    }
    0
}

#[cfg(target_os = "linux")]
fn set(cb: &mut arboard::Clipboard, text: &str) -> Result<(), arboard::Error> {
    use arboard::SetExtLinux as _;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3600);
    cb.set().wait_until(deadline).text(text)
}

#[cfg(not(target_os = "linux"))]
fn set(cb: &mut arboard::Clipboard, text: &str) -> Result<(), arboard::Error> {
    cb.set_text(text)
}

#[cfg(unix)]
fn detach(cmd: &mut Command) {
    // SAFETY: runs in the child immediately before exec; only calls
    // async-signal-safe libc functions.
    unsafe {
        use std::os::unix::process::CommandExt as _;
        cmd.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
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
