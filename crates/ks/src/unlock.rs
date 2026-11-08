//! The unlock chain: session agent first, then the passphrase. Device
//! sources (Touch ID, keychain) plug in here in M4.

use age::secrecy::SecretString;

use crate::vault::{IDENTITY_FILE, MAX_SMALL_FILE};
use crate::{Error, Key, Result, Session, Vault};

/// What the session agent offers for this vault.
#[cfg(feature = "agent")]
enum AgentProbe {
    /// No agent is running.
    Absent,
    /// An agent runs but holds a different (stale) vault key.
    Stale,
    /// The agent holds the current key; the session is ready.
    Ready(Session),
}

impl Vault {
    /// Unlocks the vault via the source chain: a running agent that passes the
    /// check-file verification, else the passphrase. `passphrase` produces the
    /// master passphrase on demand; `None` (or when it errors) means no source
    /// is available.
    ///
    /// The passphrase is authoritative: a key that opens `.identity.age` is
    /// the current key without further checks. A failing agent candidate is
    /// warned about, skipped, and asked to exit after the passphrase succeeds.
    /// If `.check.age` is missing, corrupt, or bound to another identity file
    /// it is rewritten under the write lock.
    ///
    /// # Errors
    ///
    /// - [`Error::Locked`] when no source is available.
    /// - [`Error::WrongPassphrase`], [`Error::Corrupt`], [`Error::Io`] and
    ///   [`Error::Unavailable`] as applicable.
    pub fn unlock(&self, passphrase: Option<&dyn Fn() -> Result<SecretString>>) -> Result<Session> {
        self.unlock_chain(passphrase, true)
    }

    /// Unlocks with the passphrase source only — no agent (or, in M4, device
    /// sources). Used by `unlock`, `passwd`, `rekey` and `--no-agent`.
    ///
    /// # Errors
    ///
    /// Same as [`Vault::unlock`].
    pub fn unlock_without_agent(
        &self,
        passphrase: Option<&dyn Fn() -> Result<SecretString>>,
    ) -> Result<Session> {
        self.unlock_chain(passphrase, false)
    }

    #[cfg_attr(
        not(feature = "agent"),
        allow(
            unused_variables,
            reason = "with_agent selects the chain only when the agent feature is on"
        )
    )]
    fn unlock_chain(
        &self,
        passphrase: Option<&dyn Fn() -> Result<SecretString>>,
        with_agent: bool,
    ) -> Result<Session> {
        // Read the identity file first so a broken vault never prompts.
        let identity_path = self.root().join(IDENTITY_FILE);
        let bytes = self.read_file(&identity_path, MAX_SMALL_FILE)?;

        #[cfg(feature = "agent")]
        let stale_agent = if with_agent {
            match self.agent_probe()? {
                AgentProbe::Absent => false,
                AgentProbe::Stale => true,
                AgentProbe::Ready(session) => return Ok(session),
            }
        } else {
            false
        };

        let Some(get_passphrase) = passphrase else {
            return Err(Error::Locked);
        };
        let secret = get_passphrase()?;
        let key = crate::passphrase::open_identity(&bytes, &secret)?;
        let mut warnings = Vec::new();
        #[cfg(feature = "agent")]
        if stale_agent {
            warnings
                .push("session agent does not hold the current vault key; ignoring it".to_owned());
        }
        if !self.verify_check(&key) {
            self.with_write_lock(|| self.write_check(&key))?;
            warnings.push("check file rebuilt".to_owned());
        }
        #[cfg(feature = "agent")]
        if stale_agent {
            drop(crate::agent::lock(self));
        }
        Ok(Session::new(
            Self::open(self.root())?,
            Key::Local(key),
            warnings,
        ))
    }

    /// Connects to the agent and classifies the result.
    #[cfg(feature = "agent")]
    fn agent_probe(&self) -> Result<AgentProbe> {
        let Some(key) = crate::agent::AgentKey::connect(self)? else {
            return Ok(AgentProbe::Absent);
        };
        if !self.verify_check(&key) {
            return Ok(AgentProbe::Stale);
        }
        Ok(AgentProbe::Ready(Session::new(
            Self::open(self.root())?,
            Key::Agent(key),
            Vec::new(),
        )))
    }
}
