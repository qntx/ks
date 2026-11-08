//! Key rotation (`ks rekey`) — the vault's only revocation measure.
//!
//! The identity file carries two keys `[K', K]` between the first write and
//! the final single-key write: at every moment every file decrypts with the
//! identities in the file, so `age -d -i` keeps working mid-rotation and a
//! rerun simply resumes. The passphrase must change — a lost device keeps
//! its git credentials and could open a new identity sealed with the old
//! passphrase.

use std::path::PathBuf;

use age::secrecy::{ExposeSecret as _, SecretString};

use crate::vault::{IDENTITY_FILE, MAX_ENTRY_FILE, decrypt_entry, encrypt_to};
use crate::{Error, Result, Vault};

/// What a finished (or resumed) rotation did.
#[derive(Debug)]
pub struct RekeyReport {
    /// `true` when the identity file already held two keys (a previous
    /// rotation had been interrupted and this run continued it).
    pub resumed: bool,
    /// Number of entries re-encrypted.
    pub entries: usize,
    /// Vault-relative paths written (for auto-commit): every entry file,
    /// `.identity.age` and `.check.age`.
    pub paths: Vec<PathBuf>,
    /// Non-fatal notes (device-source removal failures, passphrase policy).
    pub warnings: Vec<String>,
}

impl Vault {
    /// Rotates the vault key. `passphrase` opens `.identity.age` directly —
    /// no agent or device sources. `new_passphrase` is called only on a
    /// fresh run (the identity file holds one key); on a resumed run the
    /// given passphrase is already the new one.
    ///
    /// Steps: passphrase unlock, dirty-tree check (fresh runs only — a
    /// crash mid-rotation leaves the tree dirty and must still resume),
    /// decryptable-entry pre-check (before asking for a new passphrase),
    /// device-source removal, then under the write lock — two-key
    /// identity + check (fresh runs), per-entry re-encryption to the new
    /// current key, single-key identity + check.
    ///
    /// A single-key identity whose check file was not rewritten is a
    /// FINISHED rotation: a normal unlock heals the check, and running
    /// `rekey` again starts a second rotation.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidInput`] on a dirty git tree;
    /// [`Error::WeakPassphrase`] when the new passphrase fails policy or
    /// equals the old one; [`Error::Undecryptable`] listing entries the
    /// pre-check could not decrypt; [`Error::WrongPassphrase`] on a wrong
    /// passphrase.
    pub fn rekey(
        &self,
        passphrase: &SecretString,
        new_passphrase: &dyn Fn() -> Result<SecretString>,
    ) -> Result<RekeyReport> {
        let mut warnings = Vec::new();
        let identity_bytes = self.read_file(
            &self.root().join(IDENTITY_FILE),
            crate::vault::MAX_SMALL_FILE,
        )?;
        let key = crate::passphrase::open_identity(&identity_bytes, passphrase)?;
        let resumed = key.key_count() > 1;
        // A crash mid-rotation leaves the tree dirty and must still resume,
        // so the clean-tree rule applies to fresh runs only.
        if !resumed {
            let git = crate::git::Git::new(self.root(), false);
            if git.is_repo() && !git.is_clean()? {
                return Err(Error::InvalidInput(
                    "commit or sync pending changes before rotating the key".into(),
                ));
            }
        }
        // Pre-check: every entry must decrypt — after the final write the
        // old key is gone, so a file that fails now would be lost silently.
        // Runs BEFORE asking for a new passphrase.
        let names = self.list(None)?;
        let mut failed = Vec::new();
        for name in &names {
            let path = name.path_in(self.root());
            let bytes = self.read_file(&path, MAX_ENTRY_FILE)?;
            if decrypt_entry(&key, &bytes, name, &path).is_err() {
                failed.push(name.to_string());
            }
        }
        if !failed.is_empty() {
            return Err(Error::Undecryptable { entries: failed });
        }
        let new_pass = if resumed {
            passphrase.clone()
        } else {
            let p = new_passphrase()?;
            warnings.extend(crate::passphrase::check_new_passphrase(&p)?);
            if p.expose_secret() == passphrase.expose_secret() {
                return Err(Error::WeakPassphrase(
                    "the new passphrase must differ from the current one".into(),
                ));
            }
            p
        };
        // Remove device sources; "not enabled" is fine, other failures warn.
        #[cfg(feature = "agent")]
        if let Err(e) = crate::agent::lock(self) {
            warnings.push(format!("could not stop the session agent: {e}"));
        }
        if let Err(e) = crate::touchid::disable(self) {
            warnings.push(format!("could not remove Touch ID: {e}"));
        }
        #[cfg(feature = "keychain")]
        if let Err(e) = crate::keychain::disable(self) {
            warnings.push(format!("could not remove the keychain entry: {e}"));
        }
        let mut paths = vec![
            self.root().join(IDENTITY_FILE),
            self.root().join(crate::vault::CHECK_FILE),
        ];
        paths.extend(names.iter().map(|n| n.path_in(self.root())));
        let entries = names.len();
        self.with_write_lock(|| {
            let two = if resumed { key } else { key.with_new_current() };
            if !resumed {
                let bytes = crate::passphrase::seal_identity(&two, &new_pass)?;
                self.atomic_write(&self.root().join(IDENTITY_FILE), &bytes)?;
                self.write_check(&two)?;
            }
            for name in &names {
                let path = name.path_in(self.root());
                let bytes = self.read_file(&path, MAX_ENTRY_FILE)?;
                let entry = decrypt_entry(&two, &bytes, name, &path)?;
                let encrypted = encrypt_to(&two, entry.encode(name))?;
                self.atomic_write(&path, &encrypted)?;
            }
            let single = two.current_only();
            let bytes = crate::passphrase::seal_identity(&single, &new_pass)?;
            self.atomic_write(&self.root().join(IDENTITY_FILE), &bytes)?;
            self.write_check(&single)
        })?;
        Ok(RekeyReport {
            resumed,
            entries,
            paths,
            warnings,
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::panic,
        clippy::assert_is_empty,
        clippy::items_after_statements,
        clippy::shadow_unrelated,
        clippy::indexing_slicing,
        clippy::needless_collect,
        clippy::doc_markdown,
        reason = "test harness: panicking fixtures, terse assertions and indexing are intentional"
    )]

    use std::io::Read as _;

    use age::secrecy::SecretString;
    use zeroize::Zeroizing;

    use super::*;
    use crate::passphrase::{open_identity, seal_identity};
    use crate::{Entry, LocalKey, Name};

    const PASS: &str = "correct horse battery staple fixture";
    const NEW_PASS: &str = "a completely different new passphrase";

    fn vault() -> (tempfile::TempDir, crate::Session) {
        let dir = tempfile::tempdir().unwrap();
        let session = Vault::create(&dir.path().join("v"), &SecretString::from(PASS)).unwrap();
        (dir, session)
    }

    fn name(s: &str) -> Name {
        Name::parse(s).unwrap()
    }

    /// The LocalKey held by the on-disk identity file right now.
    fn on_disk_key(v: &Vault, pass: &str) -> LocalKey {
        let bytes = std::fs::read(v.root().join(IDENTITY_FILE)).unwrap();
        open_identity(&bytes, &SecretString::from(pass)).unwrap()
    }

    /// Decrypts every entry using ONLY the `age` crate: scrypt-open the
    /// identity file with `pass`, then decrypt each entry with the
    /// identities inside — what `age -d -i` does mid-rotation.
    fn assert_entries_age_only(v: &Vault, pass: &str, names: &[Name]) {
        let identity_bytes = std::fs::read(v.root().join(IDENTITY_FILE)).unwrap();
        let decryptor = age::Decryptor::new_buffered(&identity_bytes[..]).unwrap();
        let mut scrypt = age::scrypt::Identity::new(SecretString::from(pass));
        scrypt.set_max_work_factor(20);
        let scrypt_ref: &dyn age::Identity = &scrypt;
        let mut reader = decryptor.decrypt(core::iter::once(scrypt_ref)).unwrap();
        let mut text = String::new();
        reader.read_to_string(&mut text).unwrap();
        let keys: Vec<age::x25519::Identity> = text
            .lines()
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(|l| l.parse().unwrap())
            .collect();
        for n in names {
            let bytes = std::fs::read(n.path_in(v.root())).unwrap();
            let entry_decryptor = age::Decryptor::new_buffered(&bytes[..]).unwrap();
            let refs: Vec<&dyn age::Identity> = keys
                .iter()
                .map(|k| {
                    let r: &dyn age::Identity = k;
                    r
                })
                .collect();
            entry_decryptor.decrypt(refs.into_iter()).unwrap();
        }
    }

    /// True when the identity `key` still decrypts every entry file.
    fn key_reads_all(v: &Vault, key: &LocalKey, names: &[Name]) -> bool {
        names.iter().all(|n| {
            let path = n.path_in(v.root());
            std::fs::read(&path).is_ok_and(|b| decrypt_entry(key, &b, n, &path).is_ok())
        })
    }

    /// Re-encrypt `names` to `key`'s current recipient (the mid-rotation
    /// per-entry step), without touching the identity file.
    fn rotate_entries(v: &Vault, key: &LocalKey, names: &[Name]) {
        for n in names {
            let path = n.path_in(v.root());
            let bytes = std::fs::read(&path).unwrap();
            let entry = decrypt_entry(key, &bytes, n, &path).unwrap();
            let encrypted = encrypt_to(key, entry.encode(n)).unwrap();
            v.atomic_write(&path, &encrypted).unwrap();
        }
    }

    /// Builds a mid-rotation state: two-key identity sealed under NEW_PASS
    /// + matching check file; `n` of `names` re-encrypted already.
    fn mid_state(v: &Vault, old: LocalKey, names: &[Name], n: usize) -> LocalKey {
        let two = old.with_new_current();
        let sealed = seal_identity(&two, &SecretString::from(NEW_PASS)).unwrap();
        v.atomic_write(&v.root().join(IDENTITY_FILE), &sealed)
            .unwrap();
        v.write_check(&two).unwrap();
        rotate_entries(v, &two, &names[..n]);
        two
    }

    fn fixture() -> (tempfile::TempDir, crate::Session, Vec<Name>, LocalKey) {
        let (dir, session) = vault();
        session
            .set(
                &name("a/b"),
                &Entry::text(Zeroizing::new("v1".to_owned())).unwrap(),
                false,
            )
            .unwrap();
        session
            .set(
                &name("c"),
                &Entry::text(Zeroizing::new("v2".to_owned())).unwrap(),
                false,
            )
            .unwrap();
        let names = vec![name("a/b"), name("c")];
        let old = on_disk_key(session.vault(), PASS);
        (dir, session, names, old)
    }

    #[test]
    fn fresh_rekey_then_old_key_fails() {
        let (_d, session, names, old) = fixture();
        let report = session
            .vault()
            .rekey(&SecretString::from(PASS), &|| {
                Ok(SecretString::from(NEW_PASS))
            })
            .unwrap();
        assert!(!report.resumed);
        assert_eq!(report.entries, 2);
        assert_eq!(report.paths.len(), 4);
        // new identity has one key; old passphrase is dead
        let new_key = on_disk_key(session.vault(), NEW_PASS);
        assert_eq!(new_key.key_count(), 1);
        assert!(!key_reads_all(session.vault(), &old, &names));
        assert!(key_reads_all(session.vault(), &new_key, &names));
        assert!(session.vault().verify_check(&new_key));
        open_identity(
            &std::fs::read(session.vault().root().join(IDENTITY_FILE)).unwrap(),
            &SecretString::from(PASS),
        )
        .unwrap_err();
    }

    #[test]
    fn same_passphrase_rejected() {
        let (_d, session, _, _) = fixture();
        let e = session
            .vault()
            .rekey(&SecretString::from(PASS), &|| Ok(SecretString::from(PASS)))
            .unwrap_err();
        assert!(matches!(e, Error::WeakPassphrase(_)), "{e}");
    }

    #[test]
    fn admin_rekey_resume_a_after_identity_write() {
        let (_d, session, names, old) = fixture();
        // (a) two-key identity + check written, no entry touched
        let two = mid_state(session.vault(), old, &names, 0);
        assert_entries_age_only(session.vault(), NEW_PASS, &names);
        assert!(key_reads_all(session.vault(), &two, &names));
        let report = session
            .vault()
            .rekey(&SecretString::from(NEW_PASS), &|| {
                unreachable!("no new passphrase on resume")
            })
            .unwrap();
        assert!(report.resumed);
        let new_key = on_disk_key(session.vault(), NEW_PASS);
        assert_eq!(new_key.key_count(), 1);
        assert!(key_reads_all(session.vault(), &new_key, &names));
    }

    #[test]
    fn admin_rekey_resume_b_half_entries() {
        let (_d, session, names, old) = fixture();
        // (b) first entry rotated, second still old-key
        let two = mid_state(session.vault(), old, &names, 1);
        assert_entries_age_only(session.vault(), NEW_PASS, &names);
        assert!(key_reads_all(session.vault(), &two, &names));
        let report = session
            .vault()
            .rekey(&SecretString::from(NEW_PASS), &|| {
                unreachable!("no new passphrase on resume")
            })
            .unwrap();
        assert!(report.resumed);
        let new_key = on_disk_key(session.vault(), NEW_PASS);
        assert!(key_reads_all(session.vault(), &new_key, &names));
    }

    #[test]
    fn admin_rekey_resume_c_all_entries_two_keys() {
        let (_d, session, names, old) = fixture();
        // (c) every entry rotated; identity still holds two keys
        let two = mid_state(session.vault(), old, &names, names.len());
        assert_entries_age_only(session.vault(), NEW_PASS, &names);
        assert!(key_reads_all(session.vault(), &two, &names));
        let report = session
            .vault()
            .rekey(&SecretString::from(NEW_PASS), &|| {
                unreachable!("no new passphrase on resume")
            })
            .unwrap();
        assert!(report.resumed);
        let new_key = on_disk_key(session.vault(), NEW_PASS);
        assert!(key_reads_all(session.vault(), &new_key, &names));
        assert!(session.vault().verify_check(&new_key));
    }

    #[test]
    fn admin_rekey_resume_d_single_key_stale_check() {
        let (_d, session, names, old) = fixture();
        // (d) entries rotated + single-key identity written, but the check
        // file still belongs to the two-key identity
        let two = mid_state(session.vault(), old, &names, names.len());
        let single = two.current_only();
        let sealed = seal_identity(&single, &SecretString::from(NEW_PASS)).unwrap();
        session
            .vault()
            .atomic_write(&session.vault().root().join(IDENTITY_FILE), &sealed)
            .unwrap();
        // no write_check here — check is stale now
        assert_entries_age_only(session.vault(), NEW_PASS, &names);
        // one key -> treated as a fresh run; the callback must supply a new
        // passphrase and the rotation completes again
        const NEW_PASS2: &str = "yet another distinct passphrase 3";
        let report = session
            .vault()
            .rekey(&SecretString::from(NEW_PASS), &|| {
                Ok(SecretString::from(NEW_PASS2))
            })
            .unwrap();
        assert!(!report.resumed);
        let new_key = on_disk_key(session.vault(), NEW_PASS2);
        assert!(key_reads_all(session.vault(), &new_key, &names));
        assert!(session.vault().verify_check(&new_key));
    }

    /// `git` with an isolated config: no system/global files, fixed
    /// identity, main as the default branch.
    fn git(root: &std::path::Path, args: &[&str]) {
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(root)
            .args([
                "-c",
                "user.email=rekey@test",
                "-c",
                "user.name=rekey",
                "-c",
                "commit.gpgsign=false",
                "-c",
                "init.defaultBranch=main",
            ])
            .args(args)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", root.join(".git-test-global-config"))
            .env("GIT_TERMINAL_PROMPT", "0")
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?} failed");
    }

    #[test]
    fn admin_rekey_resume_on_dirty_git_tree() {
        let (_d, session, names, old) = fixture();
        let root = session.vault().root().to_path_buf();
        git(&root, &["init"]);
        git(&root, &["add", "-A"]);
        git(&root, &["commit", "-q", "-m", "init"]);
        // interrupted state (b): two-key identity, first entry rotated —
        // nothing committed, so the tree IS dirty
        let two = mid_state(session.vault(), old, &names, 1);
        assert!(key_reads_all(session.vault(), &two, &names));
        let report = session
            .vault()
            .rekey(&SecretString::from(NEW_PASS), &|| {
                unreachable!("no new passphrase on resume")
            })
            .unwrap();
        assert!(report.resumed);
        let new_key = on_disk_key(session.vault(), NEW_PASS);
        assert_eq!(new_key.key_count(), 1);
        assert!(key_reads_all(session.vault(), &new_key, &names));
        assert!(session.vault().verify_check(&new_key));
    }

    #[test]
    fn undecryptable_entry_aborts() {
        let (_d, session, _names, _) = fixture();
        // a valid single-X25519 age file to a random foreign recipient
        let foreign = age::x25519::Identity::generate().to_public();
        let recipient: &dyn age::Recipient = &foreign;
        let encryptor = age::Encryptor::with_recipients(core::iter::once(recipient)).unwrap();
        let path = session.vault().root().join("rogue.age");
        let mut out = Vec::new();
        let mut w = encryptor.wrap_output(&mut out).unwrap();
        std::io::Write::write_all(&mut w, b"ks1 text rogue\nx").unwrap();
        w.finish().unwrap();
        std::fs::write(&path, &out).unwrap();
        let e = session
            .vault()
            .rekey(&SecretString::from(PASS), &|| {
                Ok(SecretString::from(NEW_PASS))
            })
            .unwrap_err();
        match e {
            Error::Undecryptable { entries } => assert_eq!(entries, vec!["rogue".to_owned()]),
            other => panic!("expected Undecryptable, got {other}"),
        }
        // nothing changed: old passphrase still opens the vault
        let s = session
            .vault()
            .unlock_without_agent(Some(&|| Ok(SecretString::from(PASS))))
            .unwrap();
        assert_eq!(
            s.get(&name("a/b")).unwrap().as_text().unwrap().as_str(),
            "v1"
        );
        std::fs::remove_file(&path).unwrap();
    }
}
