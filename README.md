# ks — Key Store

[![CI](https://github.com/qntx/ks/actions/workflows/ci.yml/badge.svg)](https://github.com/qntx/ks/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

A modern, local-first, git-friendly secret manager built on the
[age](https://age-encryption.org) encryption format. Designed for developers
who want to keep API tokens, SSH key passphrases and CI secrets encrypted on
disk and out of `.env` files.

## Why ks

- **Modern crypto, no PGP.** Every secret is an [`age`](https://github.com/FiloSottile/age)
  file encrypted to one or more X25519 recipients. Your private key lives in
  a single passphrase-protected `identity.age` and is fully interoperable
  with the upstream `age` / `rage` CLIs.
- **Local-first, git-friendly.** Each secret is its own `.age` file under a
  tree of your choosing (`store/github/token.age`, …). `git diff` shows
  exactly which secret changed; merge conflicts are scoped to a single key.
- **Developer workflow built in.** `ks run -- npm start` injects secrets as
  environment variables into a subprocess without ever touching disk.
  `ks inject` renders `${KS:path}` markers in a template. `ks env` emits
  shell `export` statements.
- **Memory-hygienic.** All in-flight secrets are wrapped in `Zeroizing`
  / `SecretBox` and zeroed on drop. The session cache stores an unlocked
  X25519 private key (not the user's passphrase) and expires after a
  configurable TTL (default 15 min).
- **Multi-device via plain git.** No bespoke sync server — `git push`/`pull`
  inside the store directory does it all.
- **TOTP built in.** Stash an `otpauth://` URL with `ks set --totp` and
  generate one-time codes with `ks otp`.

## Installation

```sh
cargo install --path crates/ks-cli
```

This produces a single `ks` binary (~5 MB release).

## Quick start

```sh
# Create an identity and an empty store
ks init

# Optionally, initialise a git repo inside the store
ks init --git

# Store a secret (interactive masked prompt)
ks set github/token --note "Personal Access Token"

# Read it back
ks get github/token              # prints to stdout
ks get github/token --copy       # copies to clipboard, auto-clears in 45s

# List, search, inspect
ks ls
ks find token
ks info github/token             # metadata only — never prints the value
```

## File layout

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

Override paths with `KS_DATA_DIR`, `KS_STORE_DIR`, `KS_IDENTITY`, `KS_CONFIG`.

## Developer workflow

### `ks run` — subprocess injection (à la `op run`)

```sh
# Map specific secrets to environment variable names:
ks run --env github/token=GITHUB_TOKEN --env aws/access-key=AWS_ACCESS_KEY_ID \
       -- npm test

# Or inject an entire prefix (paths become upper-snake env names):
ks run --prefix aws -- terraform apply
# -> AWS_ACCESS_KEY=…, AWS_SECRET_KEY=…
```

Secrets exist only as environment variables in the child process and are
zeroed in the parent immediately after `spawn`.

### `ks inject` — template rendering

```sh
# .env.template:
DATABASE_URL=postgres://${KS:db/url}
GITHUB_TOKEN=${KS:github/token}
STRIPE_KEY=${KS:stripe/key:test}   # ":field" extracts an extra field

ks inject -i .env.template -o .env
```

### `ks env` — shell export statements

```sh
eval "$(ks env github aws/prod --shell sh)"
ks env --shell fish | source
ks env --shell pwsh | iex
```

## Recipients and multi-device

```sh
ks identity show                     # prints "age1..." public key
ks recipients ls
ks recipients add age1xyz…           # adds a recipient and re-encrypts everything
ks recipients rm age1xyz…
```

To onboard a new device:

1. Run `ks init` on the new device. Note its public key (`ks identity show`).
2. On an already-trusted device, `ks recipients add <new-pubkey>` (re-encrypts
   the whole store).
3. `git push` / `git pull` from inside the store directory.

## TOTP

```sh
# Store an otpauth:// URL
ks set github/totp --totp <<< 'otpauth://totp/Github:me?secret=JBSWY...&issuer=Github'

# Generate the current code
ks otp github/totp                   # prints the 6-digit code
ks otp github/totp --copy            # copies, auto-clears in 30s
```

## Session, lock & unlock

```sh
ks unlock                            # prompt once, cache for `session_ttl_secs`
ks lock                              # clear the cache immediately
ks doctor                            # health-check identity, recipients, git
ks passwd                            # rotate the identity passphrase
```

## Git sync

```sh
ks git init                          # initialise the store as a git repo
ks git sync                          # add -A, commit, pull --rebase, push
ks git status
ks git log -n 20
```

For anything more involved (remotes, branching, …) just `cd` into the store
directory and use `git` directly. `ks` makes no attempt to reinvent it.

## Environment variables

| Variable             | Purpose                                                                 |
|----------------------|-------------------------------------------------------------------------|
| `KS_PASSPHRASE`      | Non-interactive passphrase (skips prompts; for CI use).                 |
| `KS_DATA_DIR`        | Override the data directory containing `identity.age` and `store/`.     |
| `KS_STORE_DIR`       | Override the store directory specifically.                              |
| `KS_IDENTITY`        | Override the identity file path.                                        |
| `KS_CONFIG`          | Override the `config.toml` location.                                    |

## Exit codes

Stable, scriptable exit codes following [sysexits.h](https://man.freebsd.org/cgi/man.cgi?sysexits):

| Code | Meaning                                                                |
|------|------------------------------------------------------------------------|
| 0    | Success                                                                |
| 1    | Generic error / failed external command                                |
| 64   | Usage error (bad path, malformed recipient, …)                         |
| 65   | Data error (corrupt JSON, bad TOML, malformed otpauth)                 |
| 66   | Missing input (no store, no identity, no secret)                       |
| 70   | Internal software error (crypto failure)                               |
| 73   | Cannot create (store/identity/secret already exists)                   |
| 75   | Transient failure (keyring backend unavailable)                        |
| 77   | Permission denied (wrong passphrase)                                   |

## Security model

| Asset              | Protected by                                                       |
|--------------------|--------------------------------------------------------------------|
| Identity at rest   | `age` scrypt (passphrase) over the bech32 X25519 secret key.       |
| Secrets at rest    | `age` X25519 recipient mode (ChaCha20-Poly1305 + HKDF).            |
| Memory             | `Zeroizing` / `SecretBox` on every secret type; cleared on drop.   |
| Session cache      | OS keyring (Credential Manager / Keychain / Secret Service) + TTL. |
| Identity file mode | `0o600` on Unix (write-then-chmod-then-rename).                    |

**Not in scope (yet):** YubiKey / PIV plugin support, post-quantum recipients
(`age-plugin-pq`). The `identity.age` format is already plugin-ready — these
just need a CLI surface.

## Library use

The `ks` crate is published independently:

```rust
use age::secrecy::SecretString;
use ks::{Config, Secret, Store, identity};

let config = Config::load()?;
let pp = SecretString::from(std::env::var("KS_PASSPHRASE")?);
let id = identity::load(&config.identity_path, pp)?;
let store = Store::open(config, id)?;

let token = store.get("github/token")?;
println!("{}", &*token.value);
```

## Crate layout

```text
crates/
├── ks/                          # library
│   src/
│   ├── agent.rs                 # OS-keyring session cache (TTL-bound)
│   ├── config.rs                # XDG paths + tunables
│   ├── crypto.rs                # age recipient/passphrase wrappers
│   ├── error.rs                 # `thiserror`-derived Error
│   ├── git.rs                   # thin wrapper over the `git` binary
│   ├── identity.rs              # create/load/rotate the X25519 identity
│   ├── path.rs                  # strict logical-path validation
│   ├── pwgen.rs                 # CSPRNG password generation
│   ├── recipient.rs             # `.recipients` parsing & atomic writes
│   ├── secret.rs                # Secret data model
│   ├── store.rs                 # Store: CRUD + re-encryption
│   ├── totp.rs                  # RFC 6238 TOTP
│   └── lib.rs
└── ks-cli/                      # `ks` binary
    src/
    ├── cli.rs                   # clap-derive command surface
    ├── clipboard.rs             # cross-platform clipboard with auto-clear
    ├── commands/                # one file per subcommand
    ├── exit.rs                  # stable sysexits.h-style codes
    ├── prompt.rs                # cliclack wrappers
    ├── terminal.rs              # coloured stderr helpers + tree printer
    └── main.rs
```

## License

Dual-licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.
