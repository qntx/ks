<!-- markdownlint-disable MD033 MD041 MD036 -->

# ks

[![Crates.io][crates-badge]][crates-url]
[![Docs.rs][docs-badge]][docs-url]
[![CI][ci-badge]][ci-url]
[![License][license-badge]][license-url]
[![Rust][rust-badge]][rust-url]

[crates-badge]: https://img.shields.io/crates/v/ks.svg
[crates-url]: https://crates.io/crates/ks
[docs-badge]: https://img.shields.io/docsrs/ks.svg
[docs-url]: https://docs.rs/ks
[ci-badge]: https://github.com/qntx/ks/actions/workflows/ci.yml/badge.svg
[ci-url]: https://github.com/qntx/ks/actions/workflows/ci.yml
[license-badge]: https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg
[license-url]: LICENSE-MIT
[rust-badge]: https://img.shields.io/badge/rust-edition%202024-orange.svg
[rust-url]: https://doc.rust-lang.org/edition-guide/

**Local-first, git-friendly secret manager built on `age` — one passphrase-protected identity, per-secret encrypted files, plain git for sync, zero PGP.**

ks keeps API tokens, SSH passphrases, TOTP seeds and CI secrets encrypted on disk and out of `.env` files. Every secret is an `age` file under a directory tree of your choosing; the developer-workflow commands (`run`, `inject`, `env`) feed those secrets straight into subprocesses, templates, or shells without ever materialising them on disk.

## Quick Start

### Install

```sh
cargo install --path crates/ks-cli
```

Produces a single `ks` binary (~5 MB release).

### CLI Usage

```sh
# Bootstrap an identity + empty store (optionally a git repo inside it)
ks init
ks init --git

# Store, read, search
ks set github/token --note "PAT"          # masked prompt
ks get github/token                       # to stdout
ks get github/token --copy                # to clipboard, auto-clear in 45s
ks ls
ks find token
ks info github/token                      # metadata only, never reveals the value

# Generate strong passwords in-place
ks gen aws/access-key -l 32 -s alphanum --copy

# TOTP from an otpauth:// URL
ks set github/totp --totp <<< 'otpauth://totp/...'
ks otp  github/totp --copy

# Developer workflow
ks run --env github/token=GITHUB_TOKEN -- npm test
ks run --prefix aws -- terraform apply        # AWS_ACCESS_KEY=…, AWS_SECRET_KEY=…
ks inject -i .env.template -o .env            # ${KS:path} markers
eval "$(ks env github aws/prod --shell sh)"   # also: --shell fish | pwsh

# Multi-device via plain git
ks identity show                              # age1… public key
ks recipients add age1xyz…                    # re-encrypts the whole store
ks git sync                                   # add -A, commit, pull --rebase, push

# Session & maintenance
ks unlock                                     # cache for `session_ttl_secs`
ks lock                                       # clear the cache
ks doctor                                     # health-check
ks passwd                                     # rotate the identity passphrase
```

### Library Usage

```rust
use ks::{Config, Store, identity};
use secrecy::SecretString;

let config = Config::load()?;
let pp = SecretString::from(std::env::var("KS_PASSPHRASE")?);
let id = identity::load(&config.identity_path, pp)?;
let store = Store::open(config, id)?;

let token = store.get("github/token")?;
println!("{}", token.value.as_str());
```

## Design

- **Modern crypto, no PGP.** Each secret is an `age` file encrypted to one or more X25519 recipients. The identity file is interoperable with upstream [`age`] / [`rage`].
- **One file per secret.** `git diff` shows exactly which key changed; merge conflicts are scoped to a single path.
- **Plain git for sync.** No bespoke server — `git push`/`pull` inside the store directory does the job. `ks git sync` is a convenience wrapper.
- **Developer workflow first-class.** `ks run` injects secrets as env vars into a subprocess, `ks inject` renders `${KS:path}` markers in templates, `ks env` emits shell exports for `sh` / `fish` / `pwsh`.
- **Memory-hygienic.** All in-flight secrets are wrapped in `Zeroizing` / `SecretBox` and zeroed on drop.
- **Session cache.** Unlocked X25519 keys (not passphrases) live in the OS keyring (Credential Manager / Keychain / Secret Service) with a TTL (default 15 min).
- **TOTP built in.** Stash `otpauth://` URLs, generate codes with `ks otp`.
- **Stable exit codes** — `sysexits.h`-style codes (`64` usage, `65` data, `66` missing, `70` software, `73` already-exists, `75` keyring unavailable, `77` wrong passphrase).
- **Strict linting** — Clippy `pedantic` + `nursery` + `correctness` (deny), zero warnings.

## File Layout

```text
$XDG_DATA_HOME/ks/
├── identity.age              # passphrase-encrypted X25519 private key (local only)
└── store/                    # git root, safe to push
    ├── .recipients           # plaintext public-key allow-list
    └── github/
        └── token.age         # age-encrypted JSON blob

$XDG_CONFIG_HOME/ks/
└── config.toml               # session_ttl_secs, clipboard_clear_secs
```

Override via `KS_DATA_DIR`, `KS_STORE_DIR`, `KS_IDENTITY`, `KS_CONFIG`. Set `KS_PASSPHRASE` for non-interactive use (CI, scripts).

## Multi-Device Onboarding

1. Run `ks init` on the new device; copy its public key (`ks identity show`).
2. On a trusted device, `ks recipients add <new-pubkey>` — every secret is re-encrypted to the union of recipients.
3. `git pull` from the new device.

To remove access for a lost device: `ks recipients rm <pubkey>` + force-rotate any leaked secrets (no cryptography can revoke past reads).

## Security

This library has **not** been independently audited. Use at your own risk.

| Asset | Protected by |
| --- | --- |
| **Identity at rest** | `age` scrypt over a bech32 X25519 secret key (`AGE-SECRET-KEY-1…`) |
| **Secrets at rest** | `age` X25519 recipient mode (ChaCha20-Poly1305 + HKDF) |
| **Memory** | `Zeroizing` / `SecretBox` on every secret-bearing type; cleared on drop |
| **Session cache** | OS keyring (Credential Manager / Keychain / Secret Service) + TTL |
| **Identity file mode** | `0o600` on Unix (write → chmod → atomic rename) |

**Not in scope yet:** YubiKey / PIV plugin (`age-plugin-yubikey`), post-quantum recipients (`age-plugin-pq`). The `identity.age` format is already plugin-ready — only the CLI surface is missing.

[`age`]: https://github.com/FiloSottile/age
[`rage`]: https://github.com/str4d/rage

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <https://www.apache.org/licenses/LICENSE-2.0>)
- MIT License ([LICENSE-MIT](LICENSE-MIT) or <https://opensource.org/licenses/MIT>)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in this project shall be dual-licensed as above, without any additional terms or conditions.

---

<div align="center">

A **[QNTX](https://qntx.fun)** open-source project.

<a href="https://qntx.fun"><img alt="QNTX" width="369" src="https://raw.githubusercontent.com/qntx/.github/main/profile/qntx-banner.svg" /></a>

<!--prettier-ignore-->
Code is law. We write both.

</div>
