//! Integration tests over a real on-disk vault: storage, unlock healing,
//! hostile files, concurrency, and the R-never-on-disk invariant.

#![allow(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::missing_assert_message,
    clippy::let_underscore_must_use,
    clippy::items_after_statements,
    clippy::shadow_unrelated,
    clippy::indexing_slicing,
    clippy::needless_collect,
    clippy::assert_is_empty,
    clippy::cast_possible_truncation,
    reason = "integration test harness: panicking on fixture setup, terse assertions and indexing are intentional"
)]

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
#[cfg(unix)]
use std::time::Instant;

use age::secrecy::SecretString;
use ks::{Entry, Error, Name, Vault};
use zeroize::Zeroizing;

const PASS: &str = "correct horse battery staple fixture";

fn passphrase() -> SecretString {
    SecretString::from(PASS)
}

fn root() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("vault");
    (dir, root)
}

fn create() -> (tempfile::TempDir, ks::Session) {
    let (dir, root) = root();
    let session = Vault::create(&root, &passphrase()).unwrap();
    (dir, session)
}

fn unlock(root: &Path) -> ks::Session {
    Vault::open(root)
        .unwrap()
        .unlock(Some(&|| Ok(passphrase())))
        .unwrap()
}

fn name(s: &str) -> Name {
    Name::parse(s).unwrap()
}

fn text(v: &str) -> Entry {
    Entry::text(Zeroizing::new(v.to_owned())).unwrap()
}

#[test]
fn create_set_get() {
    let (_d, session) = create();
    let n = name("github/token");
    session
        .set(&n, &text("secret-value\nuser: u"), false)
        .unwrap();
    let entry = session.get(&n).unwrap();
    assert_eq!(entry.as_text().unwrap().value(), "secret-value");
    assert_eq!(entry.as_text().unwrap().field("user"), Some("u"));
    let bin = name("bin");
    session
        .set(
            &bin,
            &Entry::binary(Zeroizing::new(vec![0, 159, 146, 150])).unwrap(),
            false,
        )
        .unwrap();
    assert_eq!(session.get(&bin).unwrap().payload(), &[0, 159, 146, 150]);
}

#[test]
fn set_exists_and_overwrite() {
    let (_d, session) = create();
    let n = name("a");
    session.set(&n, &text("one"), false).unwrap();
    assert!(matches!(
        session.set(&n, &text("two"), false),
        Err(Error::Exists(_))
    ));
    session.set(&n, &text("two"), true).unwrap();
    assert_eq!(session.get(&n).unwrap().as_text().unwrap().value(), "two");
}

#[test]
fn get_missing() {
    let (_d, session) = create();
    assert!(matches!(
        session.get(&name("nope")),
        Err(Error::NotFound(_))
    ));
}

#[test]
fn unlock_wrong_passphrase() {
    let (_d, session) = create();
    let root = session.vault().root().to_path_buf();
    match Vault::open(&root).unwrap().unlock(Some(&|| {
        Ok(SecretString::from("totally different passphrase"))
    })) {
        Err(Error::WrongPassphrase) => {}
        other => panic!("expected WrongPassphrase, got {other:?}"),
    }
    assert!(matches!(
        Vault::open(&root).unwrap().unlock(None),
        Err(Error::Locked)
    ));
}

#[test]
fn unlock_heals_check_file() {
    for kind in ["delete", "corrupt"] {
        let (_d, session) = create();
        let root = session.vault().root().to_path_buf();
        let check = root.join(".check.age");
        if kind == "delete" {
            std::fs::remove_file(&check).unwrap();
        } else {
            std::fs::write(&check, b"garbage").unwrap();
        }
        let session = unlock(&root);
        assert!(
            session.warnings().iter().any(|w| w.contains("check")),
            "{kind}: {:?}",
            session.warnings()
        );
        // Rewritten check validates for subsequent unlocks without warning.
        let session2 = unlock(&root);
        assert!(
            session2.warnings().is_empty(),
            "{kind}: {:?}",
            session2.warnings()
        );
    }
    // A check bound to another identity file also heals.
    let (_d, session) = create();
    let root_a = session.vault().root().to_path_buf();
    let (_d2, other) = create();
    let root_b = other.vault().root().to_path_buf();
    std::fs::copy(root_b.join(".check.age"), root_a.join(".check.age")).unwrap();
    let session = unlock(&root_a);
    assert!(session.warnings().iter().any(|w| w.contains("check")));
}

#[test]
fn change_passphrase() {
    let (_d, session) = create();
    let root = session.vault().root().to_path_buf();
    session.set(&name("a"), &text("v"), false).unwrap();
    session
        .change_passphrase(&SecretString::from("a brand new memorable passphrase 9"))
        .unwrap();
    drop(session);
    match Vault::open(&root)
        .unwrap()
        .unlock(Some(&|| Ok(passphrase())))
    {
        Err(Error::WrongPassphrase) => {}
        other => panic!("old passphrase must fail: {other:?}"),
    }
    let session = Vault::open(&root)
        .unwrap()
        .unlock(Some(&|| {
            Ok(SecretString::from("a brand new memorable passphrase 9"))
        }))
        .unwrap();
    assert!(session.warnings().is_empty());
    assert_eq!(
        session.get(&name("a")).unwrap().as_text().unwrap().value(),
        "v"
    );
}

#[test]
fn rename_and_remove() {
    let (_d, session) = create();
    session
        .set(&name("dir/old"), &text("v\nk: x"), false)
        .unwrap();
    session
        .rename(&name("dir/old"), &name("other/new"), false)
        .unwrap();
    assert!(matches!(
        session.get(&name("dir/old")),
        Err(Error::NotFound(_))
    ));
    let moved = session.get(&name("other/new")).unwrap();
    assert_eq!(moved.as_text().unwrap().value(), "v");
    session.set(&name("keep"), &text("k"), false).unwrap();
    session.remove(&name("other/new")).unwrap();
    assert!(matches!(
        session.get(&name("other/new")),
        Err(Error::NotFound(_))
    ));
    // Empty parent dir pruned, sibling dir kept.
    assert!(!session.vault().root().join("other").exists());
    assert!(matches!(
        session.remove(&name("missing")),
        Err(Error::NotFound(_))
    ));
}

#[test]
fn rename_same_name() {
    let (_d, session) = create();
    session.set(&name("a"), &text("v"), false).unwrap();
    assert!(matches!(
        session.rename(&name("a"), &name("a"), true),
        Err(Error::InvalidInput(_))
    ));
    assert_eq!(
        session.get(&name("a")).unwrap().as_text().unwrap().value(),
        "v"
    );
}

#[test]
fn list_rules() {
    let (_d, session) = create();
    for n in ["b", "a/z", "a/y", "a/b/c"] {
        session.set(&name(n), &text("v"), false).unwrap();
    }
    let all: Vec<String> = session
        .vault()
        .list(None)
        .unwrap()
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(all, ["a/b/c", "a/y", "a/z", "b"]);
    let under_a: Vec<String> = session
        .vault()
        .list(Some(&name("a")))
        .unwrap()
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(under_a, ["a/b/c", "a/y", "a/z"]);
    // Ignored files: dot-files, non-.age, invalid names.
    let root = session.vault().root();
    std::fs::write(root.join("README.txt"), b"x").unwrap();
    std::fs::write(root.join(".hidden.age"), b"x").unwrap();
    std::fs::write(root.join("UPPER.age"), b"x").unwrap();
    let all = session.vault().list(None).unwrap();
    assert_eq!(all.len(), 4);
}

#[test]
fn forgery() {
    let (_d, session) = create();
    let root = session.vault().root().to_path_buf();
    session.set(&name("a"), &text("va"), false).unwrap();
    session.set(&name("b"), &text("vb"), false).unwrap();

    // 1. A file encrypted to a foreign recipient -> Corrupt.
    let foreign = age::x25519::Identity::generate().to_public();
    let foreign: &dyn age::Recipient = &foreign;
    let enc = age::Encryptor::with_recipients(std::iter::once(foreign)).unwrap();
    let mut out = Vec::new();
    let mut w = enc.wrap_output(&mut out).unwrap();
    w.write_all(b"ks1 text forged\npwned").unwrap();
    w.finish().unwrap();
    let forged = root.join("forged.age");
    std::fs::write(&forged, &out).unwrap();
    assert!(matches!(
        session.get(&name("forged")),
        Err(Error::Corrupt { .. })
    ));

    // 2. Swapping two legitimate files -> Corrupt (name binding).
    let pa = root.join("a.age");
    let pb = root.join("b.age");
    let tmp = root.join(".tmpswap");
    std::fs::rename(&pa, &tmp).unwrap();
    std::fs::rename(&pb, &pa).unwrap();
    std::fs::rename(&tmp, &pb).unwrap();
    assert!(matches!(
        session.get(&name("a")),
        Err(Error::Corrupt { .. })
    ));

    // 3. A file with two recipients -> Corrupt.
    let r1: &dyn age::Recipient = &age::x25519::Identity::generate().to_public();
    let r2: &dyn age::Recipient = &age::x25519::Identity::generate().to_public();
    let enc = age::Encryptor::with_recipients([r1, r2].into_iter()).unwrap();
    let mut out = Vec::new();
    let mut w = enc.wrap_output(&mut out).unwrap();
    w.write_all(b"ks1 text multi\nx").unwrap();
    w.finish().unwrap();
    std::fs::write(root.join("multi.age"), &out).unwrap();
    assert!(matches!(
        session.get(&name("multi")),
        Err(Error::Corrupt { .. })
    ));

    // 4. A scrypt-encrypted entry -> Corrupt, never a passphrase prompt.
    let mut recip = age::scrypt::Recipient::new(SecretString::from("x"));
    recip.set_work_factor(10);
    let recip: &dyn age::Recipient = &recip;
    let enc = age::Encryptor::with_recipients(std::iter::once(recip)).unwrap();
    let mut out = Vec::new();
    let mut w = enc.wrap_output(&mut out).unwrap();
    w.write_all(b"ks1 text scrypt\nx").unwrap();
    w.finish().unwrap();
    std::fs::write(root.join("scrypt.age"), &out).unwrap();
    assert!(matches!(
        session.get(&name("scrypt")),
        Err(Error::Corrupt { .. })
    ));
}

#[cfg(unix)]
#[test]
fn hostile_symlinks_and_size() {
    use std::os::unix::fs::symlink;
    let (_d, session) = create();
    let root = session.vault().root().to_path_buf();
    session.set(&name("real"), &text("v"), false).unwrap();

    // Symlinked entry file.
    symlink(root.join("real.age"), root.join("linked.age")).unwrap();
    assert!(matches!(
        session.get(&name("linked")),
        Err(Error::Corrupt { .. })
    ));

    // Symlinked directory containing a legit-looking entry.
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(
        outside.path().join("entry.age"),
        b"ks1 text linkdir/entry\nx",
    )
    .unwrap();
    symlink(outside.path(), root.join("linkdir")).unwrap();
    assert!(matches!(
        session.get(&name("linkdir/entry")),
        Err(Error::Corrupt { .. })
    ));

    // Sparse file over 64 MiB: rejected by size before reading.
    let big = root.join("big.age");
    std::fs::File::create(&big)
        .unwrap()
        .set_len(70 * 1024 * 1024)
        .unwrap();
    let start = Instant::now();
    assert!(matches!(
        session.get(&name("big")),
        Err(Error::Corrupt { .. })
    ));
    assert!(start.elapsed().as_secs() < 2);
}

#[test]
fn concurrent_writes() {
    let (_d, session) = create();
    let session = Arc::new(session);
    std::thread::scope(|s| {
        for t in 0..2 {
            let session = Arc::clone(&session);
            s.spawn(move || write_thread(&session, t));
        }
    });
    for t in 0..2 {
        for i in 0..100 {
            let n = name(&format!("t{t}/entry{i}"));
            match session.get(&n) {
                Ok(e) => assert_eq!(e.as_text().unwrap().value(), format!("v{t}-{i}")),
                Err(err) => {
                    let p = n.path_in(session.vault().root());
                    let bytes = std::fs::read(&p).unwrap_or_default();
                    panic!(
                        "{err:?} head={:?}",
                        String::from_utf8_lossy(&bytes[..bytes.len().min(300)])
                    );
                }
            }
        }
    }
}

#[test]
fn recipient_never_on_disk() {
    let (_d, session) = create();
    let root = session.vault().root().to_path_buf();
    session.set(&name("a/b"), &text("v"), false).unwrap();

    // Recover K with the official path: decrypt .identity.age as a passphrase
    // file, parse the identity lines, derive the recipient.
    let identity_bytes = std::fs::read(root.join(".identity.age")).unwrap();
    let decryptor = age::Decryptor::new_buffered(&identity_bytes[..]).unwrap();
    let mut scrypt_id = age::scrypt::Identity::new(passphrase());
    scrypt_id.set_max_work_factor(20);
    let id: &dyn age::Identity = &scrypt_id;
    let mut reader = decryptor.decrypt(std::iter::once(id)).unwrap();
    let mut text = String::new();
    use std::io::Read as _;
    reader.read_to_string(&mut text).unwrap();
    let recipient = text
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .find_map(|l| l.parse::<age::x25519::Identity>().ok())
        .unwrap()
        .to_public()
        .to_string();
    assert!(recipient.starts_with("age1"));

    for entry in walk(&root) {
        let bytes = std::fs::read(&entry).unwrap();
        let content = String::from_utf8_lossy(&bytes);
        assert!(!content.contains(&recipient), "{} leaks R", entry.display());
        assert!(
            !content.contains("AGE-SECRET-KEY-1"),
            "{} leaks K",
            entry.display()
        );
    }
}

fn write_thread(session: &ks::Session, t: usize) {
    for i in 0..100 {
        let n = name(&format!("t{t}/entry{i}"));
        session.set(&n, &text(&format!("v{t}-{i}")), false).unwrap();
    }
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                out.push(p);
            }
        }
    }
    out
}
