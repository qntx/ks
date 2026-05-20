//! Recipient list management.
//!
//! The recipients file is a plaintext list of age public keys (`age1…`) that
//! the store can be encrypted to. Lines starting with `#` and blank lines are
//! treated as comments. The file lives at `<store>/.recipients` so it is
//! safely git-synced alongside the encrypted secrets.

use std::io::Write as _;
use std::path::Path;
use std::str::FromStr as _;

use age::x25519;

use crate::error::{Error, Result};

/// Parses a recipients file body into a list of public keys, preserving order
/// and stripping comments.
///
/// # Errors
/// Returns [`Error::InvalidRecipient`] if any non-comment, non-empty line
/// fails to parse as an age public key.
pub fn parse(text: &str) -> Result<Vec<x25519::Recipient>> {
    let mut out = Vec::new();
    for (line_no, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let recipient = x25519::Recipient::from_str(line).map_err(|e| {
            Error::InvalidRecipient(format!("line {}: {e}", line_no.saturating_add(1)))
        })?;
        out.push(recipient);
    }
    Ok(out)
}

/// Reads and parses the recipients file at `path`.
///
/// # Errors
/// - [`Error::NoRecipients`] if the file does not exist or contains no keys.
/// - [`Error::Io`] on read failures.
/// - [`Error::InvalidRecipient`] on parse failures.
pub fn load(path: &Path) -> Result<Vec<x25519::Recipient>> {
    if !path.exists() {
        return Err(Error::NoRecipients(path.to_path_buf()));
    }
    let text = std::fs::read_to_string(path)?;
    let recipients = parse(&text)?;
    if recipients.is_empty() {
        return Err(Error::NoRecipients(path.to_path_buf()));
    }
    Ok(recipients)
}

/// Writes the recipients file to `path` atomically, with a generated header
/// comment.
///
/// # Errors
/// Returns [`Error::Io`] on any filesystem failure.
pub fn save(path: &Path, recipients: &[x25519::Recipient]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = with_extension(path, "tmp");

    {
        let mut file = std::fs::File::create(&tmp)?;
        writeln!(
            file,
            "# ks recipients — public keys allowed to decrypt this store."
        )?;
        writeln!(
            file,
            "# Add a recipient with `ks recipients add <age1...>`."
        )?;
        for r in recipients {
            writeln!(file, "{r}")?;
        }
        file.sync_all()?;
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Returns `true` if `target` is byte-equal (textual form) to any recipient in `list`.
#[must_use]
pub fn contains(list: &[x25519::Recipient], target: &x25519::Recipient) -> bool {
    let needle = target.to_string();
    list.iter().any(|r| r.to_string() == needle)
}

fn with_extension(path: &Path, ext: &str) -> std::path::PathBuf {
    let mut p = path.to_path_buf();
    let mut name = p
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("recipients")
        .to_owned();
    name.push('.');
    name.push_str(ext);
    p.set_file_name(name);
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_comments_and_blanks() {
        let id = x25519::Identity::generate();
        let pub_str = id.to_public().to_string();
        let body = format!("# comment line\n\n{pub_str}\n# trailing\n");
        let parsed = parse(&body).expect("parse");
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed.first().expect("non-empty").to_string(), pub_str);
    }

    #[test]
    fn rejects_invalid() {
        assert!(parse("not-a-pubkey").is_err());
    }

    #[test]
    fn save_then_load_roundtrip() {
        let tmp = tempdir();
        let path = tmp.join(".recipients");
        let id = x25519::Identity::generate();
        let r = id.to_public();
        save(&path, std::slice::from_ref(&r)).expect("save");
        let loaded = load(&path).expect("load");
        assert_eq!(loaded.len(), 1);
        assert_eq!(
            loaded.first().expect("non-empty").to_string(),
            r.to_string()
        );
    }

    fn tempdir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("ks-test-{}", rand::random::<u64>()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }
}
