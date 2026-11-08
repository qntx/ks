# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.8.0] — 2026-11-08

### Added

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

### Removed

- OS keychain factor on Linux: Secret Service exposes the key for the
  whole desktop session and its zbus backend pulled RUSTSEC-unmaintained
  crates.
- `mnemonic` and `key` entry kinds: the wallet milestone is deferred;
  kinds are `text` and `binary` only.
