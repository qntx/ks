//! Clipboard write with automatic timed clear.

use std::thread;
use std::time::Duration;

use arboard::Clipboard;
use ks::Result;

const CLEAR_SECS: u64 = 45;

/// Copies `value` to the system clipboard and spawns a background thread that
/// clears the clipboard after [`CLEAR_SECS`] seconds.
///
/// Returns the number of seconds before auto-clear for display purposes.
///
/// # Errors
/// Returns an error if the clipboard is unavailable on this system.
pub fn copy_with_autoclean(value: &str) -> Result<u64> {
    let mut cb =
        Clipboard::new().map_err(|e| ks::Error::Io(std::io::Error::other(e.to_string())))?;
    cb.set_text(value.to_owned())
        .map_err(|e| ks::Error::Io(std::io::Error::other(e.to_string())))?;

    let owned = value.to_owned();
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(CLEAR_SECS));
        if let Ok(mut cb2) = Clipboard::new()
            && let Ok(current) = cb2.get_text()
                && current == owned {
                    let _ = cb2.set_text(String::new());
                }
    });

    Ok(CLEAR_SECS)
}
