//! Agent-facing JSON output mode (`--json`).
//!
//! When enabled, every command emits a single JSON object on stdout and runs
//! fully non-interactively. The mode is process-global, set once from the parsed
//! `--json` flag, and consulted by command handlers, [`commands::unlock`] and the
//! prompt helpers so no interactive prompt can ever fire under `--json`.
//!
//! Convention: successful results and `{"error": "..."}` failures both go to
//! **stdout**, distinguished by the `error` key plus a non-zero exit code, so an
//! agent can parse a single stream.

use std::sync::OnceLock;

use ks::Error;
use serde_json::Value;

static JSON_MODE: OnceLock<bool> = OnceLock::new();

/// Records whether `--json` was given. Call once at startup, before dispatch.
pub fn init(json: bool) {
    JSON_MODE.set(json).ok();
}

/// Returns `true` when running in `--json` (non-interactive) mode.
#[must_use]
pub fn is_json() -> bool {
    JSON_MODE.get().copied().unwrap_or(false)
}

/// Prints a successful command result as a single pretty-printed JSON object.
pub fn emit(value: &Value) {
    if let Ok(text) = serde_json::to_string_pretty(value) {
        println!("{text}");
    }
}

/// Renders a failed command: `{"error": "..."}` on stdout in JSON mode, or the
/// usual red diagnostic on stderr otherwise.
pub fn error(err: &Error) {
    if is_json() {
        emit(&serde_json::json!({ "error": err.to_string() }));
    } else {
        crate::terminal::error(&err.to_string());
    }
}

/// Error returned when an interactive-only command is invoked under `--json`.
#[must_use]
pub fn interactive_only(command: &str) -> Error {
    Error::InvalidArgument(format!(
        "`{command}` is interactive and cannot run in --json mode"
    ))
}
