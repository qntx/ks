//! The unlock chain: session agent, then device sources (Touch ID, OS
//! keychain), then the passphrase. A device key must pass the check-file
//! verification; a stale candidate is warned about, skipped, and removed
//! after a successful passphrase unlock.

use age::secrecy::SecretString;

use crate::vault::{IDENTITY_FILE, MAX_SMALL_FILE};
use crate::{Error, Key, LocalKey, Result, Session, Vault};

/// What one device source produced for the unlock chain.
#[allow(
    dead_code,
    reason = "Key/Skip are only constructed on supported device platforms"
)]
pub(crate) enum Probe {
    /// Source not enabled or not present — silent skip.
    Absent,
    /// The source produced a key; the chain verifies the check file.
    Key(LocalKey),
    /// The source tried and failed softly (cancel, denied, read error,
    /// plugin failure) — warn and continue; never stale, never deleted.
    Skip(String),
}

/// A stale device source to remove after a passphrase unlock.
#[derive(Copy, Clone)]
enum Stale {
    #[cfg(feature = "agent")]
    Agent,
    TouchId,
    #[cfg(feature = "keychain")]
    Keychain,
}

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
        let identity_hash: [u8; 32] = {
            use sha2::Digest as _;
            sha2::Sha256::digest(&bytes).into()
        };
        let mut warnings = Vec::new();
        let mut stale = Vec::new();

        #[cfg(feature = "agent")]
        if with_agent {
            // A broken agent endpoint (permissions, ownership, socket) must
            // never block the passphrase: warn and keep going.
            match self.agent_probe(identity_hash) {
                Ok(AgentProbe::Absent) => {}
                Ok(AgentProbe::Stale) => stale.push(Stale::Agent),
                Ok(AgentProbe::Ready(session)) => return Ok(session),
                Err(e) => {
                    warnings.push(format!("the session agent could not be reached: {e}"));
                }
            }
        }

        if let Some(s) = self.apply_probe(
            identity_hash,
            crate::touchid::probe(self),
            Stale::TouchId,
            "Touch ID holds an outdated vault key; it will be removed after a passphrase unlock",
            &mut warnings,
            &mut stale,
        )? {
            return Ok(s);
        }
        #[cfg(feature = "keychain")]
        if let Some(s) = self.apply_probe(
            identity_hash,
            crate::keychain::probe(self),
            Stale::Keychain,
            "the OS keychain holds an outdated vault key; it will be removed after a passphrase unlock",
            &mut warnings,
            &mut stale,
        )? {
            return Ok(s);
        }

        let Some(get_passphrase) = passphrase else {
            return Err(Error::Locked);
        };
        let secret = get_passphrase()?;
        let key = crate::passphrase::open_identity(&bytes, &secret)?;
        if !self.verify_check(&key) {
            self.with_write_lock(|| self.write_check(&key))?;
            warnings.push("check file rebuilt".to_owned());
        }
        for src in stale {
            self.remove_stale(src, &mut warnings);
        }
        Ok(Session::new(
            Self::open(self.root())?,
            Key::Local(key),
            warnings,
            identity_hash,
        ))
    }

    /// Removes one stale device source after a passphrase unlock and
    /// records whether it worked — a failure is warned about, never hidden.
    fn remove_stale(&self, src: Stale, warnings: &mut Vec<String>) {
        match src {
            #[cfg(feature = "agent")]
            Stale::Agent => {
                warnings.push(
                    "session agent does not hold the current vault key; asking it to exit"
                        .to_owned(),
                );
                if let Err(e) = crate::agent::lock(self) {
                    warnings.push(format!("could not stop the session agent: {e}"));
                }
            }
            Stale::TouchId => match crate::touchid::disable(self) {
                Ok(_) => warnings.push(
                    "Touch ID has been removed; re-enable with `ks touchid enable`".to_owned(),
                ),
                Err(e) => warnings.push(format!(
                    "could not remove Touch ID: {e}; remove it with `ks touchid disable`"
                )),
            },
            #[cfg(feature = "keychain")]
            Stale::Keychain => match crate::keychain::disable(self) {
                Ok(_) => warnings.push(
                    "the OS keychain entry has been removed; re-enable with `ks keychain enable`"
                        .to_owned(),
                ),
                Err(e) => warnings.push(format!(
                    "could not remove the OS keychain entry: {e}; remove it with `ks keychain disable`"
                )),
            },
        }
    }

    /// Applies one device-source probe to the chain: a key passing the
    /// check-file verification becomes the session, a stale key is warned
    /// about and remembered for post-passphrase removal, soft failures only
    /// produce a warning.
    fn apply_probe(
        &self,
        identity_hash: [u8; 32],
        probe: Probe,
        stale_src: Stale,
        stale_warn: &str,
        warnings: &mut Vec<String>,
        stale: &mut Vec<Stale>,
    ) -> Result<Option<Session>> {
        match probe {
            Probe::Absent => Ok(None),
            Probe::Skip(w) => {
                warnings.push(w);
                Ok(None)
            }
            Probe::Key(k) if self.verify_check(&k) => Ok(Some(Session::new(
                Self::open(self.root())?,
                Key::Local(k),
                std::mem::take(warnings),
                identity_hash,
            ))),
            Probe::Key(_) => {
                warnings.push(stale_warn.to_owned());
                stale.push(stale_src);
                Ok(None)
            }
        }
    }

    /// Connects to the agent and classifies the result.
    #[cfg(feature = "agent")]
    fn agent_probe(&self, identity_hash: [u8; 32]) -> Result<AgentProbe> {
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
            identity_hash,
        )))
    }
}
