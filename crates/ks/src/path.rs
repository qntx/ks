//! Logical secret path validation and filesystem mapping.
//!
//! A logical path looks like `github/personal/token` and maps to the on-disk
//! file `<store>/github/personal/token.age`. Logical paths are deliberately
//! restricted to a small character set to avoid filesystem-quirk traps
//! (case-folding on macOS/Windows, path traversal, reserved Windows names).

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

/// On-disk file extension for an encrypted secret.
pub const SECRET_EXT: &str = "age";

/// Validates a logical secret path.
///
/// Rules:
/// - Non-empty, not longer than 1024 chars.
/// - Only ASCII letters, digits, `_`, `-`, `.`.
/// - `/` separates segments; no leading, trailing or consecutive slashes.
/// - No segment may be empty, `.`, or `..`.
/// - No segment may start or end with a dot.
/// - No reserved Windows segment names (`CON`, `PRN`, `AUX`, `NUL`, `COM1`-`COM9`, `LPT1`-`LPT9`).
///
/// # Errors
/// Returns [`Error::InvalidPath`] with a human-readable reason on any violation.
pub fn validate(path: &str) -> Result<()> {
    if path.is_empty() {
        return Err(Error::InvalidPath("path must not be empty".into()));
    }
    if path.len() > 1024 {
        return Err(Error::InvalidPath("path is too long (>1024)".into()));
    }
    if path.starts_with('/') || path.ends_with('/') {
        return Err(Error::InvalidPath(
            "path must not start or end with `/`".into(),
        ));
    }

    for segment in path.split('/') {
        validate_segment(segment)?;
    }

    Ok(())
}

fn validate_segment(segment: &str) -> Result<()> {
    if segment.is_empty() {
        return Err(Error::InvalidPath(
            "path must not contain empty segments (consecutive `/`)".into(),
        ));
    }
    if segment == "." || segment == ".." {
        return Err(Error::InvalidPath(
            "path may not contain `.` or `..` segments".into(),
        ));
    }
    if segment.starts_with('.') || segment.ends_with('.') {
        return Err(Error::InvalidPath(
            "segments may not start or end with `.`".into(),
        ));
    }
    if is_windows_reserved(segment) {
        return Err(Error::InvalidPath(format!(
            "`{segment}` is a reserved Windows filename"
        )));
    }
    for ch in segment.chars() {
        if !is_allowed_char(ch) {
            return Err(Error::InvalidPath(format!(
                "illegal character `{ch}` in path"
            )));
        }
    }
    Ok(())
}

const fn is_allowed_char(ch: char) -> bool {
    matches!(ch, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '-' | '.')
}

fn is_windows_reserved(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    let stem = upper.split_once('.').map_or(upper.as_str(), |(s, _)| s);
    matches!(
        stem,
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}

/// Joins a validated logical path onto a store root, appending the `.age` extension.
///
/// The path is **not** re-validated here; call [`validate`] first.
#[must_use]
pub fn to_file(store_root: &Path, logical: &str) -> PathBuf {
    let mut buf = store_root.to_path_buf();
    for segment in logical.split('/') {
        buf.push(segment);
    }
    buf.set_extension(SECRET_EXT);
    buf
}

/// Converts an on-disk path back to its logical form, stripping the `.age` extension.
///
/// Returns `None` if the path is not under `store_root`, is not a `.age` file,
/// or contains non-UTF8 segments.
#[must_use]
pub fn from_file(store_root: &Path, file: &Path) -> Option<String> {
    let rel = file.strip_prefix(store_root).ok()?;
    if rel.extension().and_then(|s| s.to_str()) != Some(SECRET_EXT) {
        return None;
    }
    let with_ext = rel.to_str()?;
    let logical = with_ext.strip_suffix(".age")?;
    let normalised = logical.replace('\\', "/");
    if normalised.is_empty() || normalised.contains("..") {
        return None;
    }
    Some(normalised)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_simple_path() {
        assert!(validate("github/token").is_ok());
        assert!(validate("aws/prod/access-key").is_ok());
        assert!(validate("single").is_ok());
        assert!(validate("a.b/c_d/e-f").is_ok());
    }

    #[test]
    fn rejects_empty_and_separator_edges() {
        assert!(validate("").is_err());
        assert!(validate("/foo").is_err());
        assert!(validate("foo/").is_err());
        assert!(validate("foo//bar").is_err());
    }

    #[test]
    fn rejects_traversal() {
        assert!(validate("..").is_err());
        assert!(validate("foo/../bar").is_err());
        assert!(validate("./foo").is_err());
    }

    #[test]
    fn rejects_bad_chars() {
        assert!(validate("foo bar").is_err());
        assert!(validate("foo\\bar").is_err());
        assert!(validate("foo:bar").is_err());
        assert!(validate("中文").is_err());
    }

    #[test]
    fn rejects_windows_reserved() {
        assert!(validate("con").is_err());
        assert!(validate("COM1").is_err());
        assert!(validate("foo/nul").is_err());
    }

    #[test]
    fn roundtrip_to_file() {
        let root = Path::new("/tmp/ks");
        let p = to_file(root, "github/token");
        assert_eq!(p, Path::new("/tmp/ks/github/token.age"));
        let logical = from_file(root, &p).expect("should map back");
        assert_eq!(logical, "github/token");
    }
}
