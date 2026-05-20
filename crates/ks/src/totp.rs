//! RFC 6238 TOTP code generation.
//!
//! Accepts either an `otpauth://totp/…` URL (which carries algorithm, digits,
//! step) or a bare base32-encoded secret with sensible defaults
//! (SHA-1, 6 digits, 30 s step).

use totp_rs::{Algorithm, Secret as TotpSecret, TOTP};

use crate::error::{Error, Result};

/// The result of generating a TOTP code.
#[derive(Debug, Clone)]
pub struct Code {
    /// The current numeric code (zero-padded).
    pub value: String,
    /// Seconds until the code rotates.
    pub remaining_secs: u64,
    /// Total step length in seconds.
    pub step_secs: u64,
}

/// Builds a [`TOTP`] from the stored secret material.
fn from_value(value: &str) -> Result<TOTP> {
    let trimmed = value.trim();
    if trimmed.starts_with("otpauth://") {
        TOTP::from_url(trimmed).map_err(|e| Error::InvalidTotp(e.to_string()))
    } else {
        let bytes = TotpSecret::Encoded(trimmed.to_owned())
            .to_bytes()
            .map_err(|e| Error::InvalidTotp(format!("base32 secret: {e}")))?;
        TOTP::new(Algorithm::SHA1, 6, 1, 30, bytes, None, "ks".to_owned())
            .map_err(|e| Error::InvalidTotp(e.to_string()))
    }
}

/// Generates the current TOTP [`Code`] for the given secret value.
///
/// # Errors
/// Returns [`Error::InvalidTotp`] if the value cannot be parsed or the system
/// clock cannot be queried.
pub fn current(value: &str) -> Result<Code> {
    let totp = from_value(value)?;
    let code = totp
        .generate_current()
        .map_err(|e| Error::InvalidTotp(e.to_string()))?;
    let remaining = totp.ttl().map_err(|e| Error::InvalidTotp(e.to_string()))?;
    Ok(Code {
        value: code,
        remaining_secs: remaining,
        step_secs: totp.step,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_from_base32_secret() {
        // 32 base32 chars = 20 bytes (160 bits), which exceeds the
        // 128-bit minimum enforced by RFC 6238 best practice.
        let code = current("JBSWY3DPEHPK3PXPJBSWY3DPEHPK3PXP").expect("generate");
        assert_eq!(code.value.len(), 6);
        assert!(code.value.chars().all(|c| c.is_ascii_digit()));
        assert_eq!(code.step_secs, 30);
        assert!(code.remaining_secs <= 30);
    }

    #[test]
    fn rejects_garbage() {
        assert!(current("not a secret!@#").is_err());
    }
}
