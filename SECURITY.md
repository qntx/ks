# Security Policy

ks stores secrets — passwords, API tokens, TOTP seeds, certificates, BIP-39
mnemonics and blockchain private keys — in age-encrypted files on disk. Please
do not report vulnerabilities in public issues.

Report a vulnerability by emailing **<x@qntx.org>** with:

- a description of the issue and its impact,
- steps to reproduce or a proof of concept,
- the affected versions and platforms.

We aim to acknowledge reports within 72 hours and will coordinate a fix and
disclosure timeline with you.

## Scope

In scope:

- secret material or the vault key leaking through logs, `Debug` output,
  error messages, JSON output, or memory that outlives its `Zeroizing` guard;
- forging an entry or swapping vault files in a way `ks` accepts as genuine;
- bypassing the passphrase, work-factor, or check-file verification rules;
- breaking recoverability with the official `age` CLI;
- hostile vault contents (symlinks, oversized or malformed files) crashing
  `ks` or exhausting resources;
- supply-chain problems in the dependency tree.

Out of scope (documented non-goals):

- hiding entry names (names are visible as filenames by design);
- detecting rollbacks performed through git history alone;
- malware already running as the same user while the vault is unlocked.
