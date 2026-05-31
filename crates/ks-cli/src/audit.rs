//! Optional append-only audit log, enabled with the `KS_AUDIT` environment
//! variable.
//!
//! Each invocation appends one JSON line to `<identity-dir>/logs/audit.jsonl`
//! (mode `0600` on Unix) recording *only metadata* — a timestamp, the operation,
//! the logical target path, and whether it succeeded. **No secret values are
//! ever written.** The log lives next to the local identity, never inside the
//! git-synced store, so access metadata is not pushed to remotes.
//!
//! Auditing is **off by default**; set `KS_AUDIT=1` to enable it.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ks::Config;

/// Returns `true` when `KS_AUDIT` is set to a truthy value.
fn enabled() -> bool {
    std::env::var("KS_AUDIT").is_ok_and(|v| !matches!(v.trim(), "" | "0" | "false" | "no" | "off"))
}

/// Appends one audit record for `op` on `target` (a validated logical path, or
/// empty). Best-effort: a logging failure never affects the command's outcome.
pub fn record(config: &Config, op: &str, target: &str, ok: bool) {
    if !enabled() {
        return;
    }
    append(config, op, target, ok).ok();
}

fn append(config: &Config, op: &str, target: &str, ok: bool) -> std::io::Result<()> {
    let path = log_path(config);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let record = serde_json::json!({
        "ts": ts,
        "op": op,
        "target": target,
        "result": if ok { "ok" } else { "error" },
    });
    open_append(&path)?.write_all(format!("{record}\n").as_bytes())
}

/// Audit log path: `<identity-dir>/logs/audit.jsonl`.
fn log_path(config: &Config) -> PathBuf {
    config
        .identity_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("logs")
        .join("audit.jsonl")
}

#[cfg(unix)]
fn open_append(path: &Path) -> std::io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt as _;
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(path)
}

#[cfg(not(unix))]
fn open_append(path: &Path) -> std::io::Result<std::fs::File> {
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
}
