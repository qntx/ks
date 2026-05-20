//! Cryptographically-random secret generation.
//!
//! Uses the OS-backed thread-local CSPRNG via [`rand::rng`].

use rand::seq::IndexedRandom as _;
use zeroize::Zeroizing;

/// Character set used when generating a random secret.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Charset {
    /// `a-z A-Z 0-9` (62 chars).
    #[default]
    Alphanumeric,
    /// `0-9 a-f` (16 chars).
    Hex,
    /// All printable ASCII (94 chars, excludes space and DEL).
    Printable,
    /// `a-z 0-9 -` — slug-friendly (37 chars).
    Slug,
}

impl Charset {
    const fn alphabet(self) -> &'static [u8] {
        match self {
            Self::Alphanumeric => b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789",
            Self::Hex => b"0123456789abcdef",
            Self::Printable => {
                b"!\"#$%&'()*+,-./0123456789:;<=>?@\
                  ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`\
                  abcdefghijklmnopqrstuvwxyz{|}~"
            }
            Self::Slug => b"abcdefghijklmnopqrstuvwxyz0123456789-",
        }
    }
}

impl std::str::FromStr for Charset {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "alphanum" | "alphanumeric" | "an" => Ok(Self::Alphanumeric),
            "hex" => Ok(Self::Hex),
            "printable" | "ascii" => Ok(Self::Printable),
            "slug" => Ok(Self::Slug),
            other => Err(format!(
                "unknown charset `{other}` (expected: alphanum, hex, printable, slug)"
            )),
        }
    }
}

/// Generates a random secret of `length` characters using the given [`Charset`].
///
/// # Errors
/// Returns an error if `length` is zero.
pub fn generate(length: usize, charset: Charset) -> Result<Zeroizing<String>, &'static str> {
    if length == 0 {
        return Err("length must be at least 1");
    }
    let alphabet = charset.alphabet();
    let mut rng = rand::rng();
    let mut out = String::with_capacity(length);
    for _ in 0..length {
        // Charset::alphabet() is always non-empty, but fall back to `a` to
        // stay panic-free if a future variant ever changes that.
        let byte = alphabet.choose(&mut rng).copied().unwrap_or(b'a');
        out.push(char::from(byte));
    }
    Ok(Zeroizing::new(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_correct_length() {
        for cs in [
            Charset::Alphanumeric,
            Charset::Hex,
            Charset::Printable,
            Charset::Slug,
        ] {
            let s = generate(32, cs).expect("should generate");
            assert_eq!(s.len(), 32);
        }
    }

    #[test]
    fn rejects_zero() {
        assert!(generate(0, Charset::Hex).is_err());
    }

    #[test]
    fn hex_only_contains_hex() {
        let s = generate(64, Charset::Hex).expect("should generate");
        assert!(s.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn parses_aliases() {
        assert_eq!("alphanum".parse::<Charset>(), Ok(Charset::Alphanumeric));
        assert_eq!("an".parse::<Charset>(), Ok(Charset::Alphanumeric));
        assert_eq!("slug".parse::<Charset>(), Ok(Charset::Slug));
        assert!("nope".parse::<Charset>().is_err());
    }
}
