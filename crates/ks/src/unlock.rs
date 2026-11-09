//! The unlock chain: session agent, then device sources (Touch ID, OS
//! keychain), then the passphrase. A device key must pass the check-file
//! verification; a stale candidate is warned about, skipped, and removed
//! after a successful passphrase unlock.

use age::secrecy::SecretString;

use crate::vault::{IDENTITY_FILE, MAX_SMALL_FILE};
use crate::{Error, Key, LocalKey, Result, Session, Vault};

/// Outcome of [`Vault::unlock_devices`]: a device-backed session, or
/// nothing usable with the stale marker the UI needs.
#[allow(
    missing_debug_implementations,
    reason = "Session has no Debug; the variants stay opaque"
)]
pub enum DeviceUnlock {
    /// A device source produced the session.
    Unlocked(Session),
    /// No device source could unlock; `stale` is set when any source held
    /// an outdated key (a passphrase unlock will remove it).
    Unavailable {
        /// Soft failures collected while probing (cancelled prompts,
        /// plugin errors); surfaced for logging, never toasts.
        warnings: Vec<String>,
        /// Any source held an outdated key — a passphrase unlock will
        /// remove it (DEVICE-STALE).
        stale: bool,
    },
}

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

    /// Unlocks with the passphrase and device sources — no agent. Used by
    /// `unlock`, `passwd`, `rekey` and `--no-agent`.
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
        let (bytes, identity_hash) = self.read_identity()?;
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

        if let Some(s) = self.probe_devices(identity_hash, &mut warnings, &mut stale)? {
            return Ok(s);
        }

        self.passphrase_tail(passphrase, &bytes, identity_hash, warnings, stale)
    }

    /// Reads `.identity.age` and its SHA-256 for check verification —
    /// read first so a broken vault never prompts.
    fn read_identity(&self) -> Result<(Vec<u8>, [u8; 32])> {
        let identity_path = self.root().join(IDENTITY_FILE);
        let bytes = self.read_file(&identity_path, MAX_SMALL_FILE)?;
        let hash: [u8; 32] = {
            use sha2::Digest as _;
            sha2::Sha256::digest(&bytes).into()
        };
        Ok((bytes, hash))
    }

    /// Runs the device probes — Touch ID, then the OS keychain — through
    /// `apply_probe` and returns the session a passing key produced.
    fn probe_devices(
        &self,
        identity_hash: [u8; 32],
        warnings: &mut Vec<String>,
        stale: &mut Vec<Stale>,
    ) -> Result<Option<Session>> {
        if let Some(s) = self.apply_probe(
            identity_hash,
            crate::touchid::probe(self),
            Stale::TouchId,
            "Touch ID holds an outdated vault key; it will be removed after a passphrase unlock",
            warnings,
            stale,
        )? {
            return Ok(Some(s));
        }
        #[cfg(feature = "keychain")]
        if let Some(s) = self.apply_probe(
            identity_hash,
            crate::keychain::probe(self),
            Stale::Keychain,
            "the OS keychain holds an outdated vault key; it will be removed after a passphrase unlock",
            warnings,
            stale,
        )? {
            return Ok(Some(s));
        }
        Ok(None)
    }

    /// Passphrase tail of the unlock chain: open `.identity.age`, rebuild
    /// the check file when it fails verification, remove sources proven
    /// stale by the probes, and produce the session.
    fn passphrase_tail(
        &self,
        passphrase: Option<&dyn Fn() -> Result<SecretString>>,
        bytes: &[u8],
        identity_hash: [u8; 32],
        mut warnings: Vec<String>,
        stale: Vec<Stale>,
    ) -> Result<Session> {
        let Some(get_passphrase) = passphrase else {
            return Err(Error::Locked);
        };
        let secret = get_passphrase()?;
        let key = crate::passphrase::open_identity(bytes, &secret)?;
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

    /// Unlocks with the passphrase only — no agent and no device probes;
    /// the check file is rebuilt as the chain does.
    ///
    /// # Errors
    ///
    /// [`Error::WrongPassphrase`], [`Error::Corrupt`], [`Error::Io`].
    pub fn unlock_passphrase(
        &self,
        passphrase: &dyn Fn() -> Result<SecretString>,
    ) -> Result<Session> {
        let (bytes, identity_hash) = self.read_identity()?;
        self.passphrase_tail(
            Some(passphrase),
            &bytes,
            identity_hash,
            Vec::new(),
            Vec::new(),
        )
    }

    /// Tries the device sources only — Touch ID, then the OS keychain.
    /// Never asks for a passphrase and never removes anything.
    ///
    /// # Errors
    ///
    /// Hard errors only: a corrupt identity file or I/O. Soft source
    /// failures land in [`DeviceUnlock::Unavailable::warnings`].
    pub fn unlock_devices(&self) -> Result<DeviceUnlock> {
        let (_, identity_hash) = self.read_identity()?;
        let mut warnings = Vec::new();
        let mut stale = Vec::new();
        if let Some(s) = self.probe_devices(identity_hash, &mut warnings, &mut stale)? {
            return Ok(DeviceUnlock::Unlocked(s));
        }
        Ok(DeviceUnlock::Unavailable {
            warnings,
            stale: !stale.is_empty(),
        })
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
