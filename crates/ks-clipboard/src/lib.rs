//! Secret-aware clipboard helpers shared by `ks` (CLI helper process)
//! and the desktop app.
//!
//! [`copy`] writes a secret to the system clipboard with the privacy
//! markers the platform offers (macOS: `NSPasteboardContentsCurrentHostOnly`,
//! `org.nspasteboard.ConcealedType`, `org.nspasteboard.TransientType`;
//! Windows: excluded from history and cloud sync). [`clear_if_unchanged`]
//! clears later only when nobody else wrote the clipboard in between —
//! it never reads the secret back on macOS (the `changeCount` token is
//! compared) and compares a SHA-256 of the text everywhere else.
//!
//! The caller owns timing: keep the returned [`Copied`] — and the
//! process — alive for the clipboard lifetime (on Linux the selection
//! is served by a background thread inside arboard's `Clipboard`).

#![deny(unsafe_code, reason = "objc2 FFI lives only inside the macos module")]
#![warn(missing_docs)]

#[cfg(target_os = "macos")]
mod macos;
#[cfg(not(target_os = "macos"))]
mod other;

/// Token returned by [`copy`]; proves the clipboard still holds the
/// value this crate wrote. macOS stores the pasteboard `changeCount`,
/// other platforms the SHA-256 of the value — never the value itself.
#[derive(Debug)]
#[allow(
    missing_copy_implementations,
    reason = "the non-macOS token owns a Mutex<Clipboard> and cannot be Copy"
)]
pub struct Copied(Inner);

#[cfg(target_os = "macos")]
type Inner = macos::Token;
#[cfg(not(target_os = "macos"))]
type Inner = other::Token;

/// Clipboard errors. Every variant maps to `unavailable` on the CLI.
#[derive(Debug, thiserror::Error)]
#[allow(
    clippy::error_impl_error,
    reason = "the crate API name is Error by design"
)]
pub enum Error {
    /// The clipboard service is missing or rejected the operation.
    #[error("clipboard unavailable: {0}")]
    Unavailable(String),
}

/// Writes `text` to the clipboard with platform privacy markers.
///
/// # Errors
///
/// [`Error::Unavailable`] when no clipboard service exists or a write
/// was refused.
pub fn copy(text: &str) -> Result<Copied, Error> {
    #[cfg(target_os = "macos")]
    return Ok(Copied(macos::copy(text)?));
    #[cfg(not(target_os = "macos"))]
    Ok(Copied(other::copy(text)?))
}

/// Clears the clipboard iff it still holds what [`copy`] wrote.
/// Returns `true` when it cleared, `false` when someone else wrote
/// since — in that case the foreign content is left alone.
///
/// # Errors
///
/// [`Error::Unavailable`] when the clipboard cannot be read/cleared.
pub fn clear_if_unchanged(copied: &Copied) -> Result<bool, Error> {
    #[cfg(target_os = "macos")]
    return macos::clear_if_unchanged(copied.0);
    #[cfg(not(target_os = "macos"))]
    other::clear_if_unchanged(&copied.0)
}

#[cfg(not(target_os = "macos"))]
fn token_for(text: &str) -> [u8; 32] {
    use sha2::Digest as _;
    sha2::Sha256::digest(text.as_bytes()).into()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, reason = "test assertions")]
    #[test]
    #[cfg(not(target_os = "macos"))]
    fn token_is_hash_of_value() {
        assert_ne!(super::token_for("a"), super::token_for("b"));
        assert_eq!(super::token_for("x"), super::token_for("x"));
    }
}
