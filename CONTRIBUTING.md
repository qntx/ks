# Contributing to ks

ks is a personal local secret vault: one library crate (`crates/ks`) plus a
CLI crate (`crates/ks-cli`, binary `ks`). Every entry is a standard
age-encrypted file; the whole vault can always be recovered with the official
`age` CLI without `ks`.

Agents: read [`AGENTS.md`](AGENTS.md) for the architecture invariants. Process
lives here.

## Principles

| Rule | Meaning |
| ---- | ------- |
| Small PRs | One problem per PR; reviewable diffs. |
| No compatibility debt | Remove obsolete paths; no dual APIs or migrations "for now". |
| Secrets hygiene | Secret material lives in `Zeroizing` / `age::secrecy` buffers with redacted `Debug`; the vault recipient is never printed or written. |
| Single recipient | Every entry file carries exactly one X25519 stanza — that is what makes it unforgeable. |
| Vectors | The committed fixture vault and name/header vectors are the format contract; changes must be deliberate. |

License: contributions are dual-licensed [MIT](LICENSE-MIT) OR
[Apache-2.0](LICENSE-APACHE) as stated in `README.md`.

## Prerequisites

- **Rust 1.99** — pinned by `rust-toolchain.toml`.
- **cargo-deny** — `cargo install cargo-deny --locked` for the supply-chain gate.
- **just** — optional; `just gate` runs the local gate below.

## Local gate

Run the full gate before opening a PR:

```bash
npx --yes @taplo/cli@0.7.0 fmt --check
npx --yes markdownlint-cli2@0.23.3 "*.md" "skills/**/*.md"
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo clippy -p ks --all-targets --no-default-features -- -D warnings
cargo clippy --workspace --all-targets --all-features --target x86_64-unknown-linux-gnu -- -D warnings
cargo clippy --workspace --all-targets --all-features --target x86_64-pc-windows-msvc -- -D warnings
cargo test --workspace --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features
cargo deny check
```

## Vectors

`vectors/names.json` pins every name-validation rule and `vectors/headers.json`
pins the entry plaintext format (`ks1 <kind> <name>`). `vectors/vault/` is a
complete fixture vault (fixed passphrase documented in `vectors/vault.json`,
one entry per kind) asserted by `crates/ks/tests/vectors.rs` and decrypted
with only the `age` crate by `crates/ks/tests/age_interop.rs` — that pair is
the format contract and the age-CLI recoverability guarantee.

Regenerate the fixture after deliberately changing the format:

```bash
cargo test -p ks -- --ignored regenerate_fixture
```

Regenerating changes what old vaults look like — treat it as a format break,
not a chore.

## Releasing

Tag `vX.Y.Z`: `release.yml` builds `ks` binaries for the supported targets and
`publish-crates.yml` publishes the `ks` library, then the `ks-cli` crate.
Versions are workspace-lockstep — both crates always share one version.
