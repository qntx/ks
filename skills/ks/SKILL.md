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

## git sync

```bash
ks init --git                          # new vault + git repo + "ks: init" commit
ks init --clone <url> --vault <dir>    # clone an existing vault
ks sync                                # commit -> pull --rebase -> push
ks sync --keep local                   # on conflict, keep this side
ks sync --keep remote                  # or the remote side
```

Every modifying command auto-commits only its own paths
(`ks: set <name>` / `ks: rm <name>` / `ks: mv <a> <b>` / `ks: passwd`).
`KS_NO_COMMIT=1` disables that for bulk imports. Use `ks git <args>...` for
raw git inside the vault (exit code passthrough).

## session agent

```bash
ks unlock --ttl 15m --max 8h     # unlock once into the agent (human step)
ks show github/token            # commands then run WITHOUT a passphrase source
ks lock                         # forget the vault key now
ks status --json                # vault, agent ttl/life, git state (no unlock)
```

After `ks unlock`, commands need no `KS_PASSPHRASE*`: the agent answers
age unwrap/wrap over a per-user socket until the idle ttl, the lifetime
max, or `ks lock`. `locked` (77) means unlock first or supply a passphrase
source. `--no-agent` bypasses it; `passwd`/`unlock`/`rekey` always need the
real passphrase (the agent never hands out the vault key).

## device unlock

```bash
ks touchid enable        # human step: Touch ID now confirms every unlock
ks keychain enable       # or store the key in the OS keychain
ks status --json         # "touchid": bool, "keychain": bool
ks touchid disable / ks keychain disable
```

A human enables a factor once; afterwards every command (AI agents
included) unlocks without a passphrase source — Touch ID pops its own
system dialog, the keychain needs nothing. On Linux the keychain is
readable while the desktop session is unlocked (warned at enable).
Device factors that hold a stale key are skipped with a warning and
removed after a passphrase unlock — re-enable them afterwards.

## key rotation

```bash
ks sync                       # on every available device first
ks rekey                      # needs a NEW passphrase (KS_NEW_PASSPHRASE_FILE)
ks sync                       # push the rotation
```

`rekey` is the ONLY revocation measure (a lost device keeps git
credentials, so the passphrase must change — `passwd` does not revoke).
It refuses a dirty tree, aborts listing any undecryptable entry, stops
the agent and removes local Touch ID/keychain state. A crashed run
resumes on the next `ks rekey` (the identity file temporarily holds two
keys). On other devices: sync, unlock with the new passphrase, then
`ks touchid enable` / `ks keychain enable` again, and revoke the lost
device's git credentials on the host.

A sync conflict exits **75** with `{"error":{"code":"conflict","entries":[...],
"kind":"content"|"delete"}}`. `content` → rerun with `--keep local` or
`--keep remote`. `delete` (one side deleted, the other modified) → resolve by
hand: `ks git pull --rebase`, `ks git rm|add <file>`, `ks git rebase --continue`.

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
