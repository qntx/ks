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
[`age`]: https://github.com/FiloSottile/age
[`rage`]: https://github.com/str4d/rage

> **Local-first, git-friendly secret manager built on [`age`].** One passphrase-protected identity, one encrypted file per secret, plain git for sync — zero PGP.

ks keeps API tokens, SSH/DB passphrases, TOTP seeds and CI secrets encrypted on disk and out of `.env` files. Encryption needs only public keys, so **storing a secret never asks for your passphrase** — and `ks run` injects them into a subprocess without ever touching disk.

## Highlights

- **Write without unlocking** — `insert`, `gen`, `rm` and `ls` need only public keys; only *reading* a secret unlocks the identity.
- **Modern crypto, zero PGP** — X25519 + ChaCha20-Poly1305 via `age`; identities and secrets interoperate with the [`age`] / [`rage`] CLIs.
- **Plain-text payloads** — first line is the value, `key: value` lines are fields; `age -d secret.age` stays human-readable, no bespoke container.
- **One file per secret** — clean `git diff`s, conflicts scoped to a single key, synced over plain git with no server.
- **Tamper-evident** — every secret is sealed in a path-bound envelope, so a relocated, swapped or rolled-back file is rejected on read.
- **Hardened by default** — secrets live in `Zeroizing` memory; the process disables core dumps, denies debuggers and (on Unix) locks pages out of swap; writes are atomic and lock-serialised.
- **Agent-friendly** — a global `--json` flag turns every command non-interactive and machine-readable.
- **Batteries included** — built-in TOTP, subprocess injection, and an optional audit log.

## Install

**macOS / Linux**

```sh
curl -fsSL https://sh.qntx.fun/ks | sh
```

**Windows** (PowerShell)

```powershell
irm https://sh.qntx.fun/ks/ps | iex
```

Or with Cargo — `cargo install ks-cli`.

## Usage

```sh
# Bootstrap an identity + empty store (add --git to init a repo inside it)
ks init

# Store, read, search  (writing never asks for your passphrase)
ks insert github/token                        # masked prompt, or pipe via stdin
ks insert github/token --multiline            # first line = value, then `key: value` lines
ks insert tls/key.p12 --binary < key.p12      # store raw bytes verbatim
ks show github/token                          # print the whole secret
ks show github/token -f user                  # print a single field
ks show github/token -c                       # copy the value, auto-clear in 45 s
ks ls                                         # tree of all secrets
ks grep token --values                        # search paths (and decrypted contents)

# Generate, organise, rotate
ks gen aws/access-key -l 32 -c                # generate, store, copy
ks mv github/token github/pat                 # rename (re-encrypts to re-bind path)
ks cp github/pat backup/pat                   # copy   (re-encrypts to re-bind path)
ks rm backup/pat

# TOTP — store an otpauth:// URL, then read codes
printf 'otpauth://totp/GitHub:alice?secret=…' | ks insert github/totp
ks otp github/totp -c

# Inject secrets into a subprocess (never hits disk)
ks run --env github/pat=GITHUB_TOKEN -- npm test
ks run --prefix aws -- terraform apply        # AWS_ACCESS_KEY=…, AWS_SECRET_KEY=…

# Sync across devices with plain git
ks identity                                   # this device's age1… public key
ks recipients add age1xyz…                    # re-encrypt the store to a new device
ks git push                                   # passthrough, runs inside the store

# Maintenance
ks doctor                                     # health-check
ks passwd                                     # rotate the identity passphrase
```

## Agent & JSON

The global `--json` flag makes any command emit one JSON object on stdout and run **fully non-interactively**: it never prompts, requires `KS_PASSPHRASE` to unlock, needs `--force` for destructive operations, and reports failures as `{"error": "…"}`.

```sh
export KS_PASSPHRASE='…'
echo -n 'ghp_xxx' | ks --json insert github/token   # {"path":"github/token","stored":true}
ks --json show github/token | jq -r .value          # ghp_xxx
```

The bundled skill [`skills/ks/SKILL.md`](skills/ks/SKILL.md) documents every command's JSON schema and the non-interactive contract.

> `show --json` prints the **plaintext** secret value — treat that output as sensitive.

## Library

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
println!("{}", store.get("github/token", &id)?.password());
```

## Storage & Configuration

```text
$XDG_DATA_HOME/ks/
├── identity.age          # passphrase-encrypted X25519 private key (local only)
├── logs/audit.jsonl      # optional metadata-only audit log (KS_AUDIT=1)
└── store/                # git root — safe to push
    ├── .age-recipients   # plaintext public-key allow-list
    ├── .ks.lock          # advisory write lock (git-ignored)
    └── github/
        └── token.age     # age envelope: path header + value + `key: value` fields
```

Secret paths are slash-separated; each segment allows ASCII letters, digits, `_`, `-` and `.` (so `aws/credentials.json` is stored intact) — never path traversal or reserved Windows names.

| Variable | Purpose |
| --- | --- |
| `KS_DIR` · `KS_STORE_DIR` · `KS_IDENTITY` | Override the store / identity paths |
| `KS_PASSPHRASE` | Non-interactive unlock (CI); read once, then scrubbed from the environment |
| `KS_CLIP_TIME` | Clipboard auto-clear delay in seconds (default `45`) |
| `KS_AUDIT` | `1` enables the append-only audit log |
| `NO_COLOR` | Disable colour (already off when output is piped) |

## Security

> **Not** independently audited — use at your own risk.

| Asset | Protected by |
| --- | --- |
| **Identity at rest** | `age` scrypt over a bech32 X25519 secret key |
| **Secrets at rest** | `age` X25519 recipient mode (ChaCha20-Poly1305 + HKDF) |
| **Integrity** | per-secret, path-bound envelope; relocation or rollback is rejected on read |
| **Memory** | `Zeroizing` on every secret-bearing type; cleared on drop |
| **Files** | `0o600` files / `0o700` dirs on Unix, created with `O_EXCL`; startup self-check warns on group/world access |
| **Process** | core dumps disabled, debugger attachment denied, pages locked out of swap (Unix); crash dumps suppressed (Windows) |
| **Concurrency** | store-wide advisory write lock; recipient rotation staged then committed |
| **Unlocked key** | never written to disk or a keyring; lives only in process memory |

**Roadmap:** YubiKey / PIV (`age-plugin-yubikey`) and post-quantum recipients (`age-plugin-pq`) — the `identity.age` format is already plugin-ready.

## Multi-Device

1. On the new device, run `ks init` and copy its public key (`ks identity`).
2. On a trusted device, `ks recipients add <new-pubkey>` — every secret is re-encrypted to the union of recipients.
3. `git pull` on the new device.

Revoke a lost device with `ks recipients rm <pubkey>`, then rotate any exposed secrets — no cryptography can revoke past reads.

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
