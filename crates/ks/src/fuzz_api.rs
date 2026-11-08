//! Internals exposed only for cargo-fuzz (`--cfg fuzzing`, set by
//! cargo-fuzz itself — never present in normal builds). Every entry point
//! must treat its input as adversarial bytes and never panic.

use std::path::Path;

use age::secrecy::SecretString;
use zeroize::Zeroizing;

use crate::{Entry, LocalKey, Name};

/// A fixed expected name for decode checks.
fn fixed_name() -> Name {
    match Name::parse("fuzz/entry") {
        Ok(n) => n,
        Err(_) => unreachable!("a literal name parses"),
    }
}

/// Decodes fuzz bytes as an entry plaintext (`ks1 <kind> <name>\n…`).
pub fn entry_decode(data: &[u8]) {
    let _ = Entry::decode(
        Zeroizing::new(data.to_vec()),
        &fixed_name(),
        Path::new("fuzz/entry.age"),
    );
}

/// Parses fuzz bytes as an entry name (UTF-8 lossy) and as a relative
/// `.age` path.
pub fn name(data: &[u8]) {
    let _ = Name::parse(&String::from_utf8_lossy(data));
    let _ = Name::from_relative(Path::new(&String::from_utf8_lossy(data).into_owned()));
}

/// Parses fuzz bytes as an `age` identity file (1–2 X25519 identities).
pub fn identity_text(data: &[u8]) {
    let text = SecretString::from(String::from_utf8_lossy(data).into_owned());
    let _ = LocalKey::from_identity_text(&text);
}

/// Runs the single-X25519 age header check on fuzz bytes.
pub fn age_header(data: &[u8]) {
    let _ = crate::header::check_single_x25519(data, Path::new("fuzz.age"));
}

/// Parses `git status --porcelain=v1 -z` unmerged entries.
pub fn git_porcelain(data: &[u8]) {
    let _ = crate::git::unmerged_paths(data);
}

/// Serves one agent connection from fuzz bytes: the frame reader, the
/// request dispatch and the response writer all see adversarial input.
#[cfg(feature = "agent")]
pub fn agent_frames(data: &[u8]) {
    let Ok(handler) = crate::agent::Handler::new(
        LocalKey::generate().to_identity_text(),
    ) else {
        return;
    };
    let stream = std::io::Cursor::new(data.to_vec());
    let activity = || {};
    let ttl = || 0_u64;
    let life = || 0_u64;
    let _ = handler.serve_connection(stream, &activity, &ttl, &life);
}
