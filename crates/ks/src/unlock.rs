//! The unlock chain. M1 has only the passphrase source; agent/Touch
//! ID/keychain plug in here in later milestones.

use age::secrecy::SecretString;

use crate::vault::{IDENTITY_FILE, MAX_SMALL_FILE, verify_check, write_check};
use crate::{Error, Result, Session, Vault};

impl Vault {
    /// Unlocks the vault. `passphrase` produces the master passphrase on
    /// demand; `None` (or when it errors) means no source is available.
    ///
    /// The passphrase is authoritative: a key that opens `.identity.age` is
    /// the current key without further checks. If `.check.age` is then
    /// missing, corrupt, or bound to another identity file, it is rewritten
    /// under the write lock and a warning is attached to the session.
    ///
    /// # Errors
    ///
    /// - [`Error::Locked`] when no passphrase source is available.
    /// - [`Error::WrongPassphrase`], [`Error::Corrupt`], [`Error::Io`] as
    ///   applicable.
    pub fn unlock(&self, passphrase: Option<&dyn Fn() -> Result<SecretString>>) -> Result<Session> {
        let Some(get_passphrase) = passphrase else {
            return Err(Error::Locked);
        };
        let secret = get_passphrase()?;
        let identity_path = self.root().join(IDENTITY_FILE);
        let bytes = self.read_file(&identity_path, MAX_SMALL_FILE)?;
        let key = crate::passphrase::open_identity(&bytes, &secret)?;
        let mut warnings = Vec::new();
        if !verify_check(&key, self.root()) {
            self.with_write_lock(|| write_check(&key, self.root()))?;
            warnings.push("check file rebuilt".to_owned());
        }
        Ok(Session::new(Self::open(self.root())?, key, warnings))
    }
}
