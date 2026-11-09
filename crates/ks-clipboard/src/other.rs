//! Windows and Linux: arboard, plus the privacy flags arboard exposes
//! on Windows; the token is the SHA-256 of the value.

use std::sync::Mutex;

use crate::{Error, token_for};

/// Hash token plus the live `Clipboard` — keeping it inside [`Copied`]
/// keeps serving the selection on X11/Wayland until the caller drops it.
pub(crate) struct Token {
    hash: [u8; 32],
    cb: Mutex<arboard::Clipboard>,
}

impl std::fmt::Debug for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Token")
    }
}

pub(crate) fn copy(text: &str) -> Result<Token, Error> {
    let mut cb = arboard::Clipboard::new().map_err(|e| Error::Unavailable(format!("{e}")))?;
    #[cfg(windows)]
    {
        use arboard::SetExtWindows as _;
        cb.set()
            .exclude_from_history()
            .exclude_from_cloud()
            .text(text)
            .map_err(|e| Error::Unavailable(format!("{e}")))?;
    }
    #[cfg(not(windows))]
    {
        cb.set_text(text)
            .map_err(|e| Error::Unavailable(format!("{e}")))?;
    }
    Ok(Token {
        hash: token_for(text),
        cb: Mutex::new(cb),
    })
}

pub(crate) fn clear_if_unchanged(token: &Token) -> Result<bool, Error> {
    let mut cb = token
        .cb
        .lock()
        .map_err(|_| Error::Unavailable("clipboard lock poisoned".into()))?;
    let now = cb
        .get_text()
        .map_err(|e| Error::Unavailable(format!("{e}")))?;
    if token_for(&now) != token.hash {
        return Ok(false);
    }
    cb.set_text(String::new())
        .map_err(|e| Error::Unavailable(format!("{e}")))?;
    drop(cb);
    Ok(true)
}
