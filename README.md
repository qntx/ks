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

ks keeps API tokens, SSH passphrases, TOTP seeds and CI secrets encrypted on disk and out of `.env` files. Every secret is an `age` file whose decrypted payload is plain text — interoperable with the `age` CLI. Encryption needs only public keys, so storing secrets never asks for your passphrase; `ks run` then feeds them straight into a subprocess without ever materialising them on disk.

## Quick Start

### Install the CLI

**Shell** (macOS / Linux):

```sh
curl -fsSL https://sh.qntx.fun/ks | sh
```

**PowerShell** (Windows):

```powershell
irm https://sh.qntx.fun/ks/ps | iex
```

Or via Cargo:

```bash
cargo install ks-cli
```

### CLI Usage

```sh
# Bootstrap an identity + empty store (optionally a git repo inside it)
ks init
ks init --git

# Store, read, search  (writing never asks for your passphrase)
ks insert github/token                        # masked single-line prompt
ks insert github/token --multiline            # first line = value, then `key: value` lines
echo 'ghp_xxx' | ks insert github/token       # from stdin (pipe)
ks show github/token                          # prints the whole secret
ks show github/token -c                       # copy primary value, auto-clear in 45s
ks show github/token -f user                  # print a single field
ks show github/token --meta                   # field names only, never values
ks edit github/token                          # open in $EDITOR
ks ls
ks grep token                                 # match by path
ks grep alice --values                        # also search decrypted contents

# Generate strong secrets
ks gen                                        # print a 32-char value
ks gen aws/access-key -l 32 -s alphanum -c    # store + copy

# TOTP — store an otpauth:// URL, then read codes
printf 'otpauth://totp/GitHub:alice?secret=...' | ks insert github/totp
ks otp github/totp -c

# Move / copy / remove  (ciphertext-only, no passphrase needed)
ks mv github/token github/pat
ks cp github/pat backup/pat
ks rm backup/pat

# Inject secrets into a subprocess (never hits disk)
ks run --env github/pat=GITHUB_TOKEN -- npm test
ks run --prefix aws -- terraform apply        # AWS_ACCESS_KEY=…, AWS_SECRET_KEY=…

# Multi-device via plain git
ks identity                                   # this device's age1… public key
ks recipients add age1xyz…                    # re-encrypts the whole store
ks git add -A && ks git commit -m sync && ks git push   # passthrough, runs in the store

# Maintenance
ks doctor                                     # health-check
ks passwd                                     # rotate the identity passphrase
```

### Library Usage

```rust
use age::secrecy::SecretString;
use ks::{Config, Secret, Store, crypto};

let config = Config::load()?;

// Writing needs only the public recipients — no passphrase.
let store = Store::open(config.clone())?;
store.set("github/token", &Secret::new("ghp_xxx\nuser: alice"))?;

// Reading needs the unlocked identity.
let pp = SecretString::from(std::env::var("KS_PASSPHRASE")?);
let id = crypto::load_identity(&config.identity_path, pp)?;
let token = store.get("github/token", &id)?;
println!("{}", token.password());
```

## Design

- **Modern crypto, no PGP.** Each secret is an `age` file encrypted to one or more X25519 recipients. The identity file is interoperable with upstream [`age`] / [`rage`].
- **Plain-text secrets.** The decrypted payload is just text — first line is the value, `key: value` lines are fields, the rest is free-form. `age -d secret.age` is human-readable; no bespoke container.
- **Write without unlocking.** Encryption needs only the public recipients, so `insert`, `gen`, `mv`, `cp`, `rm` and `ls` never prompt for a passphrase — only reading plaintext does.
- **One file per secret.** `git diff` shows exactly which key changed; merge conflicts are scoped to a single path.
- **Plain git for sync.** No bespoke server — `ks git …` is a thin passthrough that runs `git` inside the store directory.
- **Developer workflow first-class.** `ks run` injects secrets as env vars into a subprocess without ever touching disk; `ks edit` round-trips a secret through your `$EDITOR`.
- **Memory-hygienic.** All in-flight secrets are wrapped in `Zeroizing` and zeroed on drop.
- **No daemon, no config file, few dependencies.** Configuration is environment variables (`pass`-style); the unlocked key is never persisted.
- **TOTP built in.** Stash `otpauth://` URLs, generate codes with `ks otp`.
- **Stable exit codes** — `sysexits.h`-style codes (`64` usage, `65` data, `66` missing, `70` software, `73` already-exists, `77` wrong passphrase).
- **Strict linting** — Clippy `pedantic` + `nursery` + `correctness` (deny), zero warnings.

## File Layout

```text
$XDG_DATA_HOME/ks/
├── identity.age              # passphrase-encrypted X25519 private key (local only)
└── store/                    # git root, safe to push
    ├── .age-recipients       # plaintext public-key allow-list
    └── github/
        └── token.age         # age file; plaintext = first-line value + `key: value` fields
```

Secret paths are slash-separated logical names; each segment may contain ASCII letters, digits, `_`, `-` and `.` — so dotted names like `aws/credentials.json` are stored intact — but never path traversal or reserved Windows names.

Override paths via `KS_DIR`, `KS_STORE_DIR`, `KS_IDENTITY`. Set `KS_PASSPHRASE` for non-interactive use (CI, scripts) and `KS_CLIP_TIME` to change the clipboard auto-clear delay (default 45 s). Colour is emitted only to interactive terminals and honours [`NO_COLOR`](https://no-color.org), so piped output (e.g. `ks ls | cat`) stays plain text.

## Multi-Device Onboarding

1. Run `ks init` on the new device; copy its public key (`ks identity`).
2. On a trusted device, `ks recipients add <new-pubkey>` — every secret is re-encrypted to the union of recipients.
3. `git pull` from the new device.

To remove access for a lost device: `ks recipients rm <pubkey>` + force-rotate any leaked secrets (no cryptography can revoke past reads).

## Security

This library has **not** been independently audited. Use at your own risk.

| Asset | Protected by |
| --- | --- |
| **Identity at rest** | `age` scrypt over a bech32 X25519 secret key (`AGE-SECRET-KEY-1…`) |
| **Secrets at rest** | `age` X25519 recipient mode (ChaCha20-Poly1305 + HKDF) |
| **Memory** | `Zeroizing` on every secret-bearing type; cleared on drop |
| **Identity & secret file mode** | `0o600` on Unix (write → chmod → atomic rename) |
| **Unlocked key** | never written to disk or OS keyring; lives only in process memory |

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
