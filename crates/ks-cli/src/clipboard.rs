//! Clipboard write with automatic timed clear.

use std::thread;
use std::time::Duration;

use arboard::Clipboard;
use ks::{Error, Result};

/// Copies `value` to the system clipboard and spawns a background thread that
/// clears the clipboard after `clear_secs` seconds **only if** the clipboard
/// still holds the value we wrote (i.e. we do not stomp on user-chosen content).
///
/// Returns the configured `clear_secs` for display.
///
/// # Errors
/// Returns [`Error::Io`] if the system clipboard is unavailable.
pub fn copy_with_autoclear(value: &str, clear_secs: u64) -> Result<u64> {
    let mut cb = Clipboard::new().map_err(|e| Error::Io(std::io::Error::other(e.to_string())))?;
    cb.set_text(value.to_owned())
        .map_err(|e| Error::Io(std::io::Error::other(e.to_string())))?;

    let owned = value.to_owned();
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(clear_secs));
        if let Ok(mut cb2) = Clipboard::new()
            && let Ok(current) = cb2.get_text()
            && current == owned
        {
            let _ = cb2.set_text(String::new());
        }
    });

    Ok(clear_secs)
}
