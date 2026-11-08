//! `vectors/` conformance: name rules and the committed fixture vault.
//! `regenerate_fixture` (ignored) rebuilds `vectors/vault`. Header vectors are
//! asserted against `Entry::decode` in `entry.rs` unit tests.

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

use age::secrecy::SecretString;
use ks::{Entry, Name, Vault};
use zeroize::Zeroizing;

fn vectors() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vectors")
}

#[test]
fn name_vectors() {
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(vectors().join("names.json")).unwrap())
            .unwrap();
    for name in json["valid"].as_array().unwrap() {
        let name = name.as_str().unwrap();
        assert!(Name::parse(name).is_ok(), "valid name rejected: {name}");
    }
    for case in json["invalid"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let reason = case["reason"].as_str().unwrap();
        assert!(
            Name::parse(name).is_err(),
            "invalid name accepted: {name} ({reason})"
        );
    }
}

#[test]
fn fixture_vault() {
    let meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(vectors().join("vault.json")).unwrap())
            .unwrap();
    let passphrase = SecretString::from(meta["passphrase"].as_str().unwrap());
    let vault = Vault::open(&vectors().join("vault")).unwrap();
    let session = vault.unlock(Some(&|| Ok(passphrase.clone()))).unwrap();
    for entry in meta["entries"].as_array().unwrap() {
        let name = Name::parse(entry["name"].as_str().unwrap()).unwrap();
        let kind = entry["kind"].as_str().unwrap();
        let got = session.get(&name).unwrap();
        assert_eq!(got.kind().as_str(), kind, "{}", entry["name"]);
        match kind {
            "binary" => {
                let expected: Vec<u8> = (0..entry["hex"].as_str().unwrap().len() / 2)
                    .map(|i| {
                        u8::from_str_radix(&entry["hex"].as_str().unwrap()[i * 2..i * 2 + 2], 16)
                            .unwrap()
                    })
                    .collect();
                assert_eq!(got.payload(), &expected[..]);
            }
            _ => {
                assert_eq!(
                    got.as_text().unwrap().as_str(),
                    entry["payload"].as_str().unwrap()
                );
            }
        }
    }
}

/// Regenerates `vectors/vault` and `vectors/vault.json` from scratch:
/// `cargo test -p ks --test vectors -- --ignored regenerate_fixture`
#[test]
#[ignore = "fixture generation: run only to regenerate vectors/vault"]
fn regenerate_fixture() {
    let passphrase = SecretString::from("fixture passphrase: correct-horse-7f3a9c2e");
    let dir = vectors().join("vault");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).unwrap();
    }
    let session = Vault::create(&dir, &passphrase).unwrap();
    let set = |name: &str, entry: Entry| {
        session
            .set(&Name::parse(name).unwrap(), &entry, false)
            .unwrap();
    };
    set(
        "github/token",
        Entry::text(Zeroizing::new(
            "ghp_fixture0\nuser: octocat\nurl: https://github.com".to_owned(),
        ))
        .unwrap(),
    );
    set(
        "bin/blob",
        Entry::binary(Zeroizing::new(vec![0xde, 0xad, 0xbe, 0xef, 0x00, 0x01])).unwrap(),
    );
    set("seed", Entry::mnemonic(Zeroizing::new("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about\npassphrase: fixture".to_owned())).unwrap());
    set(
        "eth/deployer",
        Entry::key(Zeroizing::new(
            "0101010101010101010101010101010101010101010101010101010101010101\nchain: evm"
                .to_owned(),
        ))
        .unwrap(),
    );
    std::io::stdout().write_all(b"fixture regenerated").unwrap();
}
