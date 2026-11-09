//! TOTP codes from entries: the secret comes from the `otpauth`, `totp`, or
//! `otp` field (first match), else the entry's first line. Values may be
//! `otpauth://` URIs or bare base32 secrets (SHA-1, 6 digits, 30 s).

use std::time::{SystemTime, UNIX_EPOCH};

use totp_rs::{Algorithm, Secret};

use crate::{Entry, Error, Result};

/// Computes the current TOTP code and the seconds it remains valid.
///
/// # Errors
///
/// - [`Error::InvalidInput`] when the entry has no usable secret or it is
///   malformed.
/// - [`Error::InvalidInput`] for binary entries.
pub fn code(entry: &Entry, now: SystemTime) -> Result<(String, u64)> {
    let secret = secret(entry)
        .ok_or_else(|| Error::InvalidInput("binary entry has no TOTP secret".into()))?;
    let totp = parse(secret)?;
    let now_secs = now
        .duration_since(UNIX_EPOCH)
        .map_err(|_| Error::InvalidInput("time before unix epoch".into()))?
        .as_secs();
    let token = totp.generate(now_secs);
    let valid_for = totp.next_step(now_secs).saturating_sub(now_secs);
    Ok((token.to_string(), valid_for))
}

/// True when [`code`] can produce a code for `entry`: a text entry whose
/// TOTP secret (field or first line) parses.
#[must_use]
pub fn has_secret(entry: &Entry) -> bool {
    secret(entry).is_some_and(|s| parse(s).is_ok())
}

/// Field keys that hold a TOTP secret, in lookup order.
pub const FIELDS: [&str; 3] = ["otpauth", "totp", "otp"];

/// The TOTP secret candidate: the first TOTP field, else the first line;
/// `None` for binary entries.
fn secret(entry: &Entry) -> Option<&str> {
    let text = entry.as_text()?;
    Some(
        FIELDS
            .iter()
            .find_map(|key| text.field(key))
            .unwrap_or_else(|| text.value()),
    )
}

fn parse(secret: &str) -> Result<totp_rs::Totp> {
    let invalid = |e: totp_rs::TotpError| Error::InvalidInput(format!("invalid TOTP secret: {e}"));
    if secret.starts_with("otpauth://") {
        return totp_rs::Totp::from_url(secret).map_err(invalid);
    }
    let secret = Secret::try_from_base32(secret)
        .map_err(|e| Error::InvalidInput(format!("invalid TOTP base32 secret: {e}")))?;
    totp_rs::Builder::new()
        .with_algorithm(Algorithm::SHA1)
        .with_digits(6)
        .with_step_duration(30)
        .with_secret(secret)
        .build()
        .map_err(invalid)
}

#[cfg(test)]
mod tests {
    use zeroize::Zeroizing;

    use super::*;

    fn text(s: &str) -> Entry {
        Entry::text(Zeroizing::new(s.to_owned())).unwrap()
    }

    #[test]
    fn has_secret_follows_code() {
        let field = text("hunter2\notpauth: JBSWY3DPEHPK3PXPJBSWY3DPEHPK3PXP\n");
        let bare = text("JBSWY3DPEHPK3PXPJBSWY3DPEHPK3PXP\n");
        let none = text("hunter2!\nuser: alice\n");
        let binary = Entry::binary(Zeroizing::new(vec![0, 1, 2])).unwrap();
        for (entry, expect) in [
            (&field, true),
            (&bare, true),
            (&none, false),
            (&binary, false),
        ] {
            assert_eq!(has_secret(entry), expect);
            assert_eq!(code(entry, SystemTime::now()).is_ok(), expect);
        }
    }
}
