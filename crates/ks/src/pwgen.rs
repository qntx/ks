//! Uniform password generation from four character sets.

use rand::RngExt as _;
use zeroize::Zeroizing;

use crate::{Error, Result};

const ALPHANUM: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
const HEX: &[u8] = b"0123456789abcdef";
const PRINTABLE: &[u8] =
    b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!@#$%^&*()-_=+[]{}<>?/|~";
const SLUG_CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
const SLUG_SEP: u8 = b'-';

/// Available character sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Charset {
    /// `a-zA-Z0-9` — safe for any consumer.
    Alphanum,
    /// `0-9a-f` — hex secrets.
    Hex,
    /// A broad printable set without ambiguous shell-unfriendly characters
    /// (no quotes, backslash, backtick, space, semicolon, colon, comma).
    Printable,
    /// URL/filename-safe slugs: `a-z0-9` with `-` separators — never leading,
    /// trailing, or consecutive `-`.
    Slug,
}

/// Generates a uniform random string of `len` characters (1–1024).
///
/// # Errors
///
/// [`Error::InvalidInput`] when `len` is 0 or above 1024.
pub fn generate(len: usize, charset: Charset) -> Result<Zeroizing<String>> {
    if len == 0 || len > 1024 {
        return Err(Error::InvalidInput("length must be 1-1024".into()));
    }
    let mut rng = rand::rng();
    let mut out = String::with_capacity(len);
    while out.len() < len {
        let ch = match charset {
            Charset::Slug => slug_char(&mut rng, &out, len),
            Charset::Alphanum => pick(&mut rng, ALPHANUM),
            Charset::Hex => pick(&mut rng, HEX),
            Charset::Printable => pick(&mut rng, PRINTABLE),
        };
        out.push(char::from(ch));
    }
    Ok(Zeroizing::new(out))
}

fn pick(rng: &mut rand::rngs::ThreadRng, set: &[u8]) -> u8 {
    let idx = rng.random_range(0..set.len());
    set.get(idx).copied().unwrap_or(b'a')
}

/// Slug interior characters: alnum plus `-` (rejection keeps the `-` density
/// uniform and never produces `--` or edge `-`).
/// One slug character: alnum at the edges; interior positions may also be
/// `-`, but never after another `-`.
fn slug_char(rng: &mut rand::rngs::ThreadRng, out: &str, len: usize) -> u8 {
    let pos = out.len();
    if pos == 0 || pos == len - 1 {
        return pick(rng, SLUG_CHARS);
    }
    let mut c = pick_sep(rng);
    while c == SLUG_SEP && out.as_bytes().last() == Some(&SLUG_SEP) {
        c = pick_sep(rng);
    }
    c
}

fn pick_sep(rng: &mut rand::rngs::ThreadRng) -> u8 {
    let idx = rng.random_range(0..=SLUG_CHARS.len());
    SLUG_CHARS.get(idx).copied().unwrap_or(SLUG_SEP)
}
