# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

### Fixed

- copied secrets are marked concealed/transient and kept off Universal
  Clipboard on macOS, and out of clipboard history and cloud sync on
  Windows

## [0.8.2] — 2026-11-09

### Fixed

- unlocking no longer creates an empty local state directory per
  vault; it is created only when Touch ID is enabled or an agent
  starts

## [0.8.1] — 2026-11-09

### Fixed

- `ks touchid enable` always failed with `internal error: path outside
  vault`: the Touch ID blob lives in the local state directory, which the
  vault-scoped atomic write rejected
- `show -c`/`otp -c`/`gen -c` never reached the clipboard: the hidden
  helper was spawned under the wrong subcommand name and its exit was
  ignored — copying now verifies the helper actually set the value
- a loosened agent state directory no longer blocks the passphrase:
  device probes warn and fall back, `ks status` reports `agent: null`
- `ks mv a a` is a usage error (64), not `exists` (73)
- a write that races `ks rekey`/`passwd` now fails `locked` (77)
  instead of silently writing an old-key entry

## [0.8.0] — 2026-11-08

Complete rewrite. The on-disk format, library API and CLI are new and
incompatible with 0.5.0 and earlier; there is no migration — export
secrets with the old binary and `ks set` them into a new vault.

### Added

- Format: one standard age file per entry (`github/token.age`), each with
  exactly one X25519 recipient that only ever exists in memory; the entry
  name is bound in the first plaintext line; the vault key is an age
  identity sealed with the master passphrase (`.identity.age`), so every
  entry can always be recovered with the official `age` or `rage` CLI.

- Vault storage and commands: `init`, `set`, `show`, `ls`, `rm`, `mv`,
  `passwd`, `gen`, `otp`, `run`, `edit`, `doctor` (M1).
- Git integration: `init --git`, `init --clone`, auto-commits, `ks sync`
  with conflict handling, `ks git` passthrough (M2).
- Session agent: `ks unlock`, `ks lock`, `ks status`; commands run without
  a passphrase source until the agent exits (M3).
- Device unlock: `ks touchid enable|disable` (macOS Secure Enclave via
  `age-plugin-se`), `ks keychain enable|disable` (macOS/Windows OS
  keychain); stale device sources are removed after a passphrase unlock
  (M4).
- `ks rekey`: key rotation — the only revocation measure; the passphrase
  must change, the agent is stopped and local device factors removed,
  interrupted rotations resume on the next run (M5).

### Security

- `ks sync` stages only entry files and the two git files, so a remote
  that weakens `.gitignore` cannot get other files committed; `ks doctor`
  reports and `--fix` restores tampered git files.
- Hostile vault files are rejected before decryption: symlinks, oversized
  files, scrypt work factors above 2^20, and anything other than a single
  X25519 recipient.

### Removed

- OS keychain factor on Linux: Secret Service exposes the key for the
  whole desktop session and its zbus backend pulled RUSTSEC-unmaintained
  crates.
- `mnemonic` and `key` entry kinds: the wallet milestone is deferred;
  kinds are `text` and `binary` only.
