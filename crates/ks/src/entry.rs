//! Entry plaintext format: `ks1 <kind> <name>` header plus payload.

use std::fmt;
use std::str::FromStr;

use zeroize::Zeroizing;

use crate::{Error, Name, Result};

/// Maximum size of a decrypted entry payload (64 MiB).
pub(crate) const MAX_ENTRY_PLAINTEXT: u64 = 64 * 1024 * 1024;

/// The kind of an entry payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Kind {
    /// Free-form UTF-8 text with `key: value` fields.
    Text,
    /// Arbitrary bytes.
    Binary,
    /// A BIP-39 mnemonic with an optional `passphrase` field.
    Mnemonic,
    /// A hex private key with a mandatory `chain` field.
    Key,
}

impl Kind {
    /// The wire string of the kind.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Binary => "binary",
            Self::Mnemonic => "mnemonic",
            Self::Key => "key",
        }
    }
}

impl FromStr for Kind {
    type Err = ();
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "text" => Ok(Self::Text),
            "binary" => Ok(Self::Binary),
            "mnemonic" => Ok(Self::Mnemonic),
            "key" => Ok(Self::Key),
            _ => Err(()),
        }
    }
}

/// A decrypted entry: kind plus payload. The payload is never copied out of a
/// [`Zeroizing`] buffer.
pub struct Entry {
    kind: Kind,
    payload: Zeroizing<Vec<u8>>,
}

impl Entry {
    /// A text entry. The payload must be non-empty UTF-8 without `0x0D`.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidInput`] on empty payloads or payloads containing `\r`.
    #[allow(
        clippy::needless_pass_by_value,
        reason = "owning the buffer guarantees zeroization on drop"
    )]
    pub fn text(payload: Zeroizing<String>) -> Result<Self> {
        if payload.is_empty() {
            return Err(Error::InvalidInput("entry payload is empty".into()));
        }
        if payload.contains('\r') {
            return Err(Error::InvalidInput(
                "entry payload must not contain \\r".into(),
            ));
        }
        Ok(Self {
            kind: Kind::Text,
            payload: Zeroizing::new(payload.as_bytes().to_vec()),
        })
    }

    /// A binary entry of at most 64 MiB.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidInput`] on oversized payloads.
    #[allow(
        clippy::needless_pass_by_value,
        reason = "owning the buffer guarantees zeroization on drop"
    )]
    pub fn binary(payload: Zeroizing<Vec<u8>>) -> Result<Self> {
        if payload.len() as u64 > MAX_ENTRY_PLAINTEXT {
            return Err(Error::InvalidInput("entry payload exceeds 64 MiB".into()));
        }
        Ok(Self {
            kind: Kind::Binary,
            payload,
        })
    }

    /// A mnemonic entry: first line is the BIP-39 mnemonic, optional
    /// `passphrase` field. Same text rules as [`Entry::text`].
    ///
    /// # Errors
    ///
    /// [`Error::InvalidInput`] on empty payloads or payloads containing `\r`.
    #[allow(
        clippy::needless_pass_by_value,
        reason = "owning the buffer guarantees zeroization on drop"
    )]
    pub fn mnemonic(payload: Zeroizing<String>) -> Result<Self> {
        Self::text_like(Kind::Mnemonic, Zeroizing::new(payload.as_bytes().to_vec()))
    }

    /// A private-key entry: first line is a hex key, mandatory `chain` field.
    /// Same text rules as [`Entry::text`].
    ///
    /// # Errors
    ///
    /// [`Error::InvalidInput`] on empty payloads or payloads containing `\r`.
    #[allow(
        clippy::needless_pass_by_value,
        reason = "owning the buffer guarantees zeroization on drop"
    )]
    pub fn key(payload: Zeroizing<String>) -> Result<Self> {
        Self::text_like(Kind::Key, Zeroizing::new(payload.as_bytes().to_vec()))
    }

    fn text_like(kind: Kind, payload: Zeroizing<Vec<u8>>) -> Result<Self> {
        let text = core::str::from_utf8(&payload)
            .map_err(|_| Error::InvalidInput("text payload is not UTF-8".into()))?;
        if text.is_empty() {
            return Err(Error::InvalidInput("entry payload is empty".into()));
        }
        if text.contains('\r') {
            return Err(Error::InvalidInput(
                "entry payload must not contain \\r".into(),
            ));
        }
        Ok(Self { kind, payload })
    }

    /// The entry kind.
    #[must_use]
    pub const fn kind(&self) -> Kind {
        self.kind
    }

    /// Raw payload bytes. For text kinds this is UTF-8.
    #[must_use]
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    /// Text view for `text`, `mnemonic` and `key` entries.
    #[must_use]
    pub fn as_text(&self) -> Option<Text<'_>> {
        match self.kind {
            Kind::Text | Kind::Mnemonic | Kind::Key => {
                Some(Text(core::str::from_utf8(&self.payload).unwrap_or("")))
            }
            Kind::Binary => None,
        }
    }

    /// `ks1 <kind> <name>\n` followed by the payload.
    pub(crate) fn encode(&self, name: &Name) -> Zeroizing<Vec<u8>> {
        let mut out = Vec::with_capacity(self.payload.len() + 32);
        out.extend_from_slice(b"ks1 ");
        out.extend_from_slice(self.kind.as_str().as_bytes());
        out.push(b' ');
        out.extend_from_slice(name.as_str().as_bytes());
        out.push(b'\n');
        out.extend_from_slice(&self.payload);
        Zeroizing::new(out)
    }

    /// Strictly decodes a plaintext against the expected name.
    ///
    /// # Errors
    ///
    /// [`Error::Corrupt`] on any deviation: wrong prefix, spacing, unknown
    /// kind, name mismatch, `\r`, or non-UTF-8 text.
    #[allow(
        clippy::needless_pass_by_value,
        reason = "owning the buffer guarantees zeroization on drop"
    )]
    pub(crate) fn decode(plain: Zeroizing<Vec<u8>>, expected: &Name, path: &Path) -> Result<Self> {
        let corrupt = |reason: &'static str| Error::Corrupt {
            path: path.to_path_buf(),
            reason,
        };
        let header_end = plain
            .iter()
            .position(|&b| b == b'\n')
            .ok_or_else(|| corrupt("no header line"))?;
        let header = core::str::from_utf8(plain.get(..header_end).unwrap_or_default())
            .map_err(|_| corrupt("header is not UTF-8"))?;
        let mut parts = header.splitn(3, ' ');
        if parts.next() != Some("ks1") {
            return Err(corrupt("bad magic"));
        }
        let kind: Kind = parts
            .next()
            .ok_or_else(|| corrupt("missing kind"))?
            .parse()
            .map_err(|()| corrupt("unknown kind"))?;
        let name = parts.next().ok_or_else(|| corrupt("missing name"))?;
        if name.contains(' ') {
            return Err(corrupt("extra bytes in header"));
        }
        if name != expected.as_str() {
            return Err(corrupt("name does not match path"));
        }
        let payload = Zeroizing::new(plain.get(header_end + 1..).unwrap_or_default().to_vec());
        match kind {
            Kind::Text | Kind::Mnemonic | Kind::Key => {
                let text = core::str::from_utf8(&payload).map_err(|_| corrupt("not UTF-8"))?;
                if text.contains('\r') {
                    return Err(corrupt("contains CR"));
                }
                if text.is_empty() {
                    return Err(corrupt("empty payload"));
                }
            }
            Kind::Binary => {}
        }
        Ok(Self { kind, payload })
    }
}

impl fmt::Debug for Entry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Entry")
            .field("kind", &self.kind)
            .field("payload_len", &self.payload.len())
            .finish()
    }
}

use std::path::Path;

/// Text view over a `text`, `mnemonic` or `key` payload.
#[derive(Debug, Clone, Copy)]
pub struct Text<'a>(&'a str);

impl<'a> Text<'a> {
    /// First line of the payload (the entry's primary value).
    #[must_use]
    pub fn value(&self) -> &'a str {
        self.0.split('\n').next().unwrap_or("")
    }

    /// A `key: value` field (exactly `": "` separator, non-empty key without
    /// whitespace), searched across all lines after the first.
    #[must_use]
    pub fn field(&self, key: &str) -> Option<&'a str> {
        self.fields().find(|(k, _)| *k == key).map(|(_, v)| v)
    }

    /// Iterates `key: value` field lines after the first line. Lines that do
    /// not match the field shape are free text and are skipped.
    pub fn fields(&self) -> impl Iterator<Item = (&'a str, &'a str)> + 'a {
        self.0.split('\n').skip(1).filter_map(|line| {
            let (key, value) = line.split_once(": ")?;
            if key.is_empty() || key.bytes().any(|b| b.is_ascii_whitespace()) {
                return None;
            }
            Some((key, value))
        })
    }

    /// The whole payload as text.
    #[must_use]
    pub const fn as_str(&self) -> &'a str {
        self.0
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

    fn name() -> Name {
        Name::parse("a/b").unwrap()
    }

    #[test]
    fn round_trip() {
        let n = name();
        let entry = Entry::text(Zeroizing::new("value\nuser: u\nfree text".to_owned())).unwrap();
        let decoded = Entry::decode(entry.encode(&n), &n, Path::new("x")).unwrap();
        assert_eq!(decoded.kind(), Kind::Text);
        assert_eq!(decoded.payload(), b"value\nuser: u\nfree text");
    }

    #[test]
    fn decode_rejects() {
        let n = name();
        for (plain, why) in [
            ("ks1 text a/b\n", "empty payload rejected"),
            ("ks1 text a/c\nx", "name mismatch"),
            ("ks1  text a/b\nx", "double space"),
            ("ks1 Text a/b\nx", "unknown kind"),
            ("ks2 text a/b\nx", "bad magic"),
            ("ks1 text a/b x\np", "extra header bytes"),
            ("ks1 text a/b\nx\ry", "CR rejected"),
            ("ks1 text a/b", "no newline"),
        ] {
            assert!(
                Entry::decode(
                    Zeroizing::new(plain.as_bytes().to_vec()),
                    &n,
                    Path::new("x")
                )
                .is_err(),
                "{why}: {plain:?}"
            );
        }
        // non-UTF-8 text payload
        let mut bad = b"ks1 text a/b\n\xff\xfe".to_vec();
        assert!(
            Entry::decode(Zeroizing::new(std::mem::take(&mut bad)), &n, Path::new("x")).is_err()
        );
    }

    #[test]
    fn binary_round_trip() {
        let n = name();
        let entry = Entry::binary(Zeroizing::new(vec![0, 255, 13, 10, 0])).unwrap();
        let decoded = Entry::decode(entry.encode(&n), &n, Path::new("x")).unwrap();
        assert_eq!(decoded.kind(), Kind::Binary);
        assert_eq!(decoded.payload(), &[0, 255, 13, 10, 0]);
    }

    #[test]
    fn text_fields() {
        let entry = Entry::text(Zeroizing::new(
            "v\nuser: u\nurl: https://x\nnote line\nport: 22".to_owned(),
        ))
        .unwrap();
        let t = entry.as_text().unwrap();
        assert_eq!(t.value(), "v");
        assert_eq!(t.field("user"), Some("u"));
        assert_eq!(t.field("url"), Some("https://x"));
        assert_eq!(t.field("port"), Some("22"));
        assert_eq!(t.field("missing"), None);
        assert_eq!(t.fields().count(), 3);
    }

    #[test]
    fn header_vectors() {
        let json: serde_json::Value =
            serde_json::from_str(include_str!("../../../vectors/headers.json")).unwrap();
        for case in json["entries"].as_array().unwrap() {
            let plaintext = case["plaintext"].as_str().unwrap();
            let name = Name::parse(case["name"].as_str().unwrap()).unwrap();
            let ok = case["ok"].as_bool().unwrap();
            let result = Entry::decode(
                Zeroizing::new(plaintext.as_bytes().to_vec()),
                &name,
                Path::new("vector"),
            );
            assert_eq!(result.is_ok(), ok, "{plaintext:?} ({})", case["reason"]);
            if ok {
                assert_eq!(
                    result.unwrap().kind().as_str(),
                    case["kind"].as_str().unwrap()
                );
            }
        }
    }

    proptest! {
        #[test]
        fn decode_never_panics(bytes in proptest::collection::vec(any::<u8>(), 0..1024)) {
            let _ = Entry::decode(Zeroizing::new(bytes), &name(), Path::new("x"));
        }

        #[test]
        fn encode_decode_round_trip(payload in "[ -~\n]{1,400}") {
            let n = name();
            let entry = Entry::text(Zeroizing::new(payload.clone())).unwrap();
            let decoded = Entry::decode(entry.encode(&n), &n, Path::new("x")).unwrap();
            prop_assert_eq!(decoded.payload(), payload.as_bytes());
        }
    }
}
