---
name: ks
description: Use the ks password vault CLI to store, read, generate and inject secrets non-interactively. Use when a task needs credentials from the user's local vault, or when writing secrets into it.
---

# ks — personal secret vault

`ks` is a local secret vault. Entries are age-encrypted files under `~/.ks`
(git-synced by the user). Every command accepts `--vault <dir>` and `--json`.

## Non-interactive usage (agents MUST use this)

Always run non-interactively: `--json` on stdout, the passphrase via a file:

```bash
export KS_PASSPHRASE_FILE=/path/to/pass   # one line, trailing newline ok
ks --json ls                            # {"entries":["github/token",...]}
ks --json show github/token             # {"name":...,"value":...,"fields":{...}}
ks --json show github/token -f user     # one field
ks --json show tls/key.p12              # binary -> {"base64": "..."}
```

Create a vault (needs `KS_NEW_PASSPHRASE_FILE` for the new passphrase):

```bash
export KS_NEW_PASSPHRASE_FILE=/path/to/new-pass
ks init --vault ~/.ks
```

Write secrets (never on the command line — always via stdin):

```bash
printf '%s' "$TOKEN" | ks set api/token            # one line
cat secret.txt | ks set doc -m                     # multiline
cat cert.der | ks set tls/cert -b                  # binary
ks set api/token --force < newvalue                # overwrite existing
ks gen newpass -l 48                               # generate + store
ks rm old --force && ks mv a b --force
```

Inject secrets into a child process without printing them:

```bash
ks run -e GH_TOKEN=github/token -e USER=github/token#user -- gh auth status
```

`ks otp <name>` prints a TOTP code; `ks doctor` checks vault health and
`ks doctor --fix` removes temp files and fixes permissions.

## Exit codes

0 ok · 64 usage (bad name/args, missing `--force`) · 65 corrupt or
weak_passphrase · 66 not_found · 73 exists · 74 io · 77 locked or
wrong_passphrase · 69 unavailable · 1 cancelled · 70 internal.

## Safety rules

- NEVER pass a passphrase as a CLI argument; use `KS_PASSPHRASE_FILE` /
  `KS_NEW_PASSPHRASE_FILE` files (mode 0600) or `KS_PASSPHRASE`.
- NEVER echo, log or commit secret values; pipe them through stdin.
- `-c` copies to the clipboard via a detached helper and auto-clears after
  `KS_CLIP_TIME` (default 45) seconds.
- Non-interactive shells get no prompts: an overwrite without `--force` fails
  `exists`; `rm`/`mv` without `--force` fails `usage`.
