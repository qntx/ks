//! Recovery using ONLY the `age` crate: decrypt `.identity.age` with the
//! fixture passphrase, parse it as an identity file, and decrypt every entry.
//! No `ks` APIs except `Vault`/`Name` for enumeration are used.

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

use std::io::Read as _;
use std::path::Path;

use age::secrecy::SecretString;

fn vectors() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vectors")
}

#[test]
fn recover_vault_with_age_only() {
    let root = vectors().join("vault");
    let meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(vectors().join("vault.json")).unwrap())
            .unwrap();
    let passphrase = SecretString::from(meta["passphrase"].as_str().unwrap());

    // 1. Decrypt the passphrase-sealed identity file (what `age -d -i` does).
    let identity_bytes = std::fs::read(root.join(".identity.age")).unwrap();
    let decryptor = age::Decryptor::new_buffered(&identity_bytes[..]).unwrap();
    assert!(decryptor.is_scrypt());
    let mut scrypt_id = age::scrypt::Identity::new(passphrase);
    scrypt_id.set_max_work_factor(20);
    let scrypt_ref: &dyn age::Identity = &scrypt_id;
    let mut reader = decryptor.decrypt(std::iter::once(scrypt_ref)).unwrap();
    let mut identity_text = String::new();
    reader.read_to_string(&mut identity_text).unwrap();

    // 2. Parse the identity file: ignore comments/blanks, X25519 per line.
    let keys: Vec<age::x25519::Identity> = identity_text
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.parse().unwrap())
        .collect();
    assert_eq!(keys.len(), 1);

    // 3. Decrypt every entry and check the bound header line.
    for entry in meta["entries"].as_array().unwrap() {
        let name = entry["name"].as_str().unwrap();
        let kind = entry["kind"].as_str().unwrap();
        let bytes = std::fs::read(root.join(format!("{name}.age"))).unwrap();
        let decryptor = age::Decryptor::new_buffered(&bytes[..]).unwrap();
        let key_refs: Vec<&dyn age::Identity> = keys
            .iter()
            .collect::<Vec<&age::x25519::Identity>>()
            .into_iter()
            .map(|k| {
                let r: &dyn age::Identity = k;
                r
            })
            .collect();
        let mut reader = decryptor.decrypt(key_refs.into_iter()).unwrap();
        let mut plaintext = Vec::new();
        reader.read_to_end(&mut plaintext).unwrap();
        let text = String::from_utf8_lossy(&plaintext);
        assert!(
            text.starts_with(&format!("ks1 {kind} {name}\n")),
            "{name}: bad header: {:?}",
            &text[..text.len().min(80)]
        );
    }
}
