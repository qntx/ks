//! Entry names and their mapping to paths inside the vault.

use std::fmt;
use std::path::{Path, PathBuf};

use crate::{Error, Result};

const MAX_LEN: usize = 200;
const RESERVED: &[&str] = &[
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// A validated entry name: `/`-separated segments of `[a-z0-9._-]`.
///
/// Rules: total length 1–200 bytes; no empty segment; no segment starts or ends
/// with `.`; no segment is a Windows device name (including the `name.ext`
/// form); the name does not end with `.age`.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Name(String);

impl Name {
    /// Parses and validates a name.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidName`] when any rule is violated.
    pub fn parse(s: &str) -> Result<Self> {
        if s.is_empty() || s.len() > MAX_LEN {
            return Err(Error::InvalidName {
                name: s.to_owned(),
                reason: "length must be 1-200 bytes",
            });
        }
        for segment in s.split('/') {
            check_segment(s, segment)?;
        }
        if ends_with_age(s) {
            return Err(Error::InvalidName {
                name: s.to_owned(),
                reason: "must not end with .age",
            });
        }
        Ok(Self(s.to_owned()))
    }

    /// The name as a string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The entry file path `<root>/<name>.age`.
    #[must_use]
    pub fn path_in(&self, root: &Path) -> PathBuf {
        let mut path = root.to_path_buf();
        let mut segments = self.0.split('/').peekable();
        while let Some(segment) = segments.next() {
            if segments.peek().is_none() {
                path.push(format!("{segment}.age"));
            } else {
                path.push(segment);
            }
        }
        path
    }

    /// Whether `self` equals `prefix` or sits below it segment-wise.
    ///
    /// `a/b/c` is under `a/b`; `a/b` is under `a` and under itself.
    #[must_use]
    pub fn is_under(&self, prefix: &Self) -> bool {
        if self.0 == prefix.0 {
            return true;
        }
        self.0.strip_prefix(prefix.0.as_str()).is_some_and(|tail| tail.starts_with('/'))
    }

    /// Builds a name from a path relative to the vault root, stripping a
    /// trailing `.age` extension. Returns `None` for anything that is not a
    /// valid entry path (dot-names, non-ASCII, invalid names).
    pub(crate) fn from_relative(path: &Path) -> Option<Self> {
        let mut name = String::new();
        for component in path.components() {
            let segment = component.as_os_str().to_str()?;
            if segment.starts_with('.') {
                return None;
            }
            if !name.is_empty() {
                name.push('/');
            }
            name.push_str(segment);
        }
        let name = name.strip_suffix(".age")?.to_owned();
        Self::parse(&name).ok()
    }
}

/// `.age` is a deliberately case-sensitive suffix: names are lowercase by rule,
/// so `name.AGE` is already rejected by the character check below.
#[allow(clippy::case_sensitive_file_extension_comparisons, reason = "names are guaranteed lowercase, so case folding is meaningless here")]
fn ends_with_age(s: &str) -> bool {
    s.ends_with(".age")
}

fn check_segment(full: &str, segment: &str) -> Result<()> {
    if segment.is_empty() {
        return Err(Error::InvalidName {
            name: full.to_owned(),
            reason: "empty segment",
        });
    }
    if segment.starts_with('.') || segment.ends_with('.') {
        return Err(Error::InvalidName {
            name: full.to_owned(),
            reason: "segment must not start or end with '.'",
        });
    }
    if !segment.bytes().all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'.' | b'_' | b'-')) {
        return Err(Error::InvalidName {
            name: full.to_owned(),
            reason: "only lowercase ascii, digits and ._- are allowed",
        });
    }
    let stem = segment.split('.').next().unwrap_or(segment);
    if RESERVED.contains(&stem) {
        return Err(Error::InvalidName {
            name: full.to_owned(),
            reason: "reserved Windows device name",
        });
    }
    Ok(())
}

impl fmt::Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::panic,
        clippy::missing_assert_message,
        clippy::let_underscore_must_use,
        clippy::items_after_statements,
        clippy::shadow_unrelated,
        clippy::indexing_slicing,
        clippy::needless_collect,
        reason = "unit tests may panic on fixture setup and use terse assertions"
    )]
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn valid_names() {
        for name in ["a", "github/token", "aws/credentials.json", "a-b_c.d", "x/y/z"] {
            assert!(Name::parse(name).is_ok(), "{name}");
        }
    }

    #[test]
    fn invalid_names() {
        for name in [
            "", "A", "a b", "a//b", ".a", "a.", "a/.b", "a/b.", "con", "con.txt", "COM1",
            "aux/x.y", "nul", "x.age", "a/b.age", "a\\b", "../x", "a:b", "con/.x",
        ] {
            assert!(Name::parse(name).is_err(), "{name}");
        }
        assert!(Name::parse(&"a".repeat(201)).is_err());
        assert!(Name::parse(&"a".repeat(200)).is_ok());
    }

    #[test]
    fn path_mapping() {
        let root = Path::new("/v");
        assert_eq!(Name::parse("github/token").unwrap().path_in(root), Path::new("/v/github/token.age"));
        assert_eq!(Name::parse("a/b.c").unwrap().path_in(root), Path::new("/v/a/b.c.age"));
    }

    #[test]
    fn under_prefix() {
        let n = Name::parse("a/b/c").unwrap();
        assert!(n.is_under(&Name::parse("a/b").unwrap()));
        assert!(n.is_under(&n));
        assert!(!n.is_under(&Name::parse("a/bc").unwrap()));
        assert!(!n.is_under(&Name::parse("a/b/c/d").unwrap()));
    }

    proptest! {
        #[test]
        fn parse_never_panics(s in ".*") {
            let _ = Name::parse(&s);
        }

        #[test]
        fn valid_names_round_trip(s in "[a-z]([a-z0-9._-]{0,18}[a-z0-9_-])?(/[a-z]([a-z0-9._-]{0,18}[a-z0-9_-])?){0,3}") {
            let n = Name::parse(&s).unwrap();
            prop_assert_eq!(n.as_str(), s);
        }
    }
}
