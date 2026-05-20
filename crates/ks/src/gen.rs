use rand::Rng as _;
use zeroize::Zeroizing;

/// Character set used when generating a random secret.
#[derive(Debug, Clone, Copy, Default)]
pub enum Charset {
    /// Lowercase letters, uppercase letters, and digits (`a-z A-Z 0-9`).
    #[default]
    Alphanumeric,
    /// Lowercase hexadecimal digits (`0-9 a-f`).
    Hex,
    /// All printable ASCII characters (codes 0x21–0x7E).
    Printable,
}

impl Charset {
    fn alphabet(self) -> &'static [u8] {
        match self {
            Self::Alphanumeric => {
                b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"
            }
            Self::Hex => b"0123456789abcdef",
            Self::Printable => {
                b"!\"#$%&'()*+,-./0123456789:;<=>?@\
                  ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`\
                  abcdefghijklmnopqrstuvwxyz{|}~"
            }
        }
    }
}

/// Generates a random secret of `length` characters using the given [`Charset`].
///
/// Uses the OS-backed thread-local RNG (`rand::thread_rng`) which is
/// cryptographically secure on all supported platforms.
///
/// # Panics
/// Panics if `length` is zero (no meaningful secret can be generated).
#[must_use]
pub fn generate(length: usize, charset: Charset) -> Zeroizing<String> {
    assert!(length > 0, "length must be at least 1");
    let alphabet = charset.alphabet();
    let n = alphabet.len();
    let mut rng = rand::thread_rng();
    let s: String = (0..length)
        .map(|_| alphabet[rng.gen_range(0..n)] as char)
        .collect();
    Zeroizing::new(s)
}
