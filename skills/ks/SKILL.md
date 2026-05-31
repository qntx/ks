---
name: ks
description: >-
  Local-first, age-encrypted secret manager for the terminal. Use when an agent
  needs to store, read, generate, search, rotate, or inject secrets — API tokens,
  SSH/DB passphrases, TOTP seeds, or whole credential files — from the command
  line. Fully non-interactive and scriptable: export KS_PASSPHRASE and pass
  --json for a single machine-readable JSON object per command.
---

# ks — Local-First Secret Manager

`ks` is a single-binary CLI that keeps secrets in per-secret [`age`](https://age-encryption.org)-encrypted files under one passphrase-protected X25519 identity, synced with plain git. Encryption needs only public keys, so **writing never needs the passphrase**; only reading (and rotating recipients) unlocks the identity. Each secret is sealed in a versioned envelope bound to its logical path, so a relocated or tampered file is rejected on read.

## Installation

**macOS / Linux:**

```sh
curl -fsSL https://sh.qntx.fun/ks | sh
```

**Windows (PowerShell):**

```powershell
irm https://sh.qntx.fun/ks/ps | iex
```

Or via Cargo: `cargo install ks-cli`. Verify with `ks --version`.

## Agent Setup — read this first

1. **`--json` is global and means "fully non-interactive".** Put it anywhere: `ks --json show github/token`. Output is a single JSON object on **stdout**, no colors, no prompts.
2. **Unlocking requires `KS_PASSPHRASE`.** Any command that reads plaintext (`show`, `grep --values`, `otp`, `run`, `identity`, `recipients add/rm`, `mv`, `cp`) or creates the identity (`init`) needs it. In `--json` mode the passphrase **must** come from `KS_PASSPHRASE`; there is no prompt. It is read once and then scrubbed from the process environment.
3. **Destructive operations need `--force`** in `--json` mode: `rm`, and overwriting an existing secret via `insert`/`gen`. Without `--force` they return an error instead of prompting.
4. **Provide secret values on stdin** (pipe). `insert` reads stdin in `--json` mode.
5. **Errors** are `{"error": "<message>"}` on stdout with a non-zero exit code.
6. **`edit` and `passwd` are interactive-only** and return an error in `--json` mode.

```sh
export KS_PASSPHRASE='…'           # required for reads / init in --json mode
echo -n 'ghp_xxx' | ks --json insert github/token
TOKEN=$(ks --json show github/token | jq -r .value)
```

## CLI Structure

```text
ks [--json] <command> [options]
```

| Command                    | Aliases | Description                                                                              | Unlocks?         |
| -------------------------- | ------- | ---------------------------------------------------------------------------------------- | ---------------- |
| `init [--git]`             |         | Create identity + store (optionally `git init`)                                          | creates identity |
| `insert <path>`            | `set`   | Store/overwrite a secret. Flags: `-m/--multiline`, `-b/--binary`, `-f/--force`           | no               |
| `gen [path]`               |         | Generate a random value. Flags: `-l/--length`, `-s/--charset`, `-f/--force`, `-c/--copy` | no               |
| `show <path>`              | `get`   | Print a secret. Flags: `-f/--field <k>`, `--meta`, `-c/--copy`                           | yes              |
| `ls [prefix]`              | `list`  | List secret paths                                                                        | no               |
| `grep <query>`             | `find`  | Search paths; `--values` also searches decrypted content                                 | with `--values`  |
| `otp <path>`               |         | Generate a TOTP code. `-c/--copy`                                                        | yes              |
| `rm <path>`                | `del`   | Remove a secret. `-f/--force`                                                            | no               |
| `mv <from> <to>`           |         | Rename (re-binds path → re-encrypts)                                                     | yes              |
| `cp <from> <to>`           |         | Copy (re-binds path → re-encrypts)                                                       | yes              |
| `run -e P=NAME … -- <cmd>` |         | Run a subprocess with secrets injected as env vars                                       | yes              |
| `recipients ls\|add\|rm`   |         | Manage the recipient public-key list                                                     | add/rm           |
| `identity`                 |         | Print this device's `age1…` public key                                                   | yes              |
| `doctor`                   |         | Health-check the store                                                                   | optional         |
| `git <args…>`              |         | Run git inside the store (passthrough)                                                   | no               |
| `edit <path>`              |         | Edit in `$EDITOR` (**interactive only**)                                                 | yes              |
| `passwd`                   |         | Rotate the identity passphrase (**interactive only**)                                    | yes              |

## Common Flags

| Flag          | Short | Scope                 | Description                                                    |
| ------------- | ----- | --------------------- | -------------------------------------------------------------- |
| `--json`      |       | global                | Machine-readable JSON, fully non-interactive                   |
| `--force`     | `-f`  | `insert`, `gen`, `rm` | Overwrite / delete without confirmation (required in `--json`) |
| `--multiline` | `-m`  | `insert`              | Read a multi-line secret from stdin until EOF                  |
| `--binary`    | `-b`  | `insert`              | Store raw bytes from stdin verbatim (no field parsing)         |
| `--field`     | `-f`  | `show`                | Operate on one `key: value` field                              |
| `--meta`      |       | `show`                | Field names only, never values                                 |
| `--length`    | `-l`  | `gen`                 | Character count (default 32)                                   |
| `--charset`   | `-s`  | `gen`                 | `alphanum` (default), `hex`, `printable`, `slug`               |
| `--values`    |       | `grep`                | Also search decrypted contents (slow)                          |
| `--env`       | `-e`  | `run`                 | `<path>=<ENV_NAME>` mapping (repeatable)                       |
| `--prefix`    | `-p`  | `run`                 | Inject every secret under a prefix (repeatable)                |

## Secret Format

A text secret is plain UTF-8: the **first line** is the primary value, subsequent `key: value` lines are queryable **fields**, and the rest is free-form. Binary secrets (`--binary`) hold arbitrary bytes with no field parsing.

```text
ghp_xxxxxxxxxxxx
user: alice
url: https://github.com
```

## JSON Schemas

Always use `--json` for programmatic use. Each command prints one object.

```jsonc
// init
{ "identity_path": "…", "store_dir": "…", "public_key": "age1…", "git": false }

// ls
{ "secrets": ["github/token", "aws/key"] }

// show  (text)
{ "path": "github/token", "kind": "text",
  "value": "ghp_xxx",                  // primary value (first line)
  "text": "ghp_xxx\nuser: alice\n",    // full plaintext
  "fields": { "user": "alice" } }
// show  (binary)
{ "path": "tls/key.p12", "kind": "binary", "base64": "MIIK…" }
// show --field user
{ "path": "github/token", "field": "user", "value": "alice" }
// show --meta   (names only, no values)
{ "path": "github/token", "fields": ["user", "url"] }

// insert / gen / rm / mv / cp
{ "path": "github/token", "stored": true }
{ "value": "0uTII7…", "length": 16, "charset": "alphanum", "stored": "rand/key" }
{ "path": "github/token", "removed": true }
{ "from": "github/token", "to": "github/pat", "moved": true }
{ "from": "github/pat", "to": "backup/pat", "copied": true }

// grep / otp / identity
{ "query": "github", "matches": ["github/token"] }
{ "path": "github/totp", "code": "123456", "valid_for_secs": 23 }
{ "public_key": "age1…" }

// recipients
{ "recipients": ["age1…", "age1…"] }
{ "added": "age1…", "reencrypted": 12 }
{ "removed": "age1…", "reencrypted": 12 }

// doctor
{ "checks": [ { "check": "identity unlocks", "ok": true, "detail": "…" } ],
  "notes": ["git: not a repository"], "failures": 0, "ok": true }

// any command on failure
{ "error": "secret not found: github/token" }
```

## Environment Variables

| Variable                                  | Purpose                                                                                                       |
| ----------------------------------------- | ------------------------------------------------------------------------------------------------------------- |
| `KS_PASSPHRASE`                           | Unlock the identity non-interactively (required in `--json` for reads/init). Scrubbed from env after reading. |
| `KS_DIR` / `KS_STORE_DIR` / `KS_IDENTITY` | Override store/identity paths                                                                                 |
| `KS_CLIP_TIME`                            | Clipboard auto-clear seconds (default 45)                                                                     |
| `KS_AUDIT`                                | `1` enables an append-only metadata audit log (`logs/audit.jsonl`)                                            |
| `NO_COLOR`                                | Disable colour (already off under `--json` / pipes)                                                           |

## Exit Codes

`sysexits.h`-style: `0` success, `1` generic, `64` usage (bad args / missing `--force`), `65` data (corrupt/tampered secret), `66` missing (store/identity/secret not found), `70` software, `73` already-exists, `77` wrong passphrase / missing `KS_PASSPHRASE`.

## Security Notes

- **`show --json` returns plaintext secret values on stdout.** Treat that output as sensitive: do not log it, echo it into shell history, or persist it. Prefer capturing into a variable (`$(… | jq -r .value)`).
- Success and error JSON both go to **stdout**; distinguish by the `error` key and the exit code.
- `run` and `git` are passthrough: the child process / `git` owns stdout, so `--json` only affects _their errors_, not their normal output.

## Agent Best Practices

1. **Always pass `--json`** for programmatic use; parse stdout as one JSON object.
2. **Export `KS_PASSPHRASE`** before reads/`init`; it is required (no prompt) and auto-scrubbed.
3. **Pipe secret values on stdin** for `insert`; add `--multiline` for `key: value` bodies, `--binary` for raw bytes.
4. **Use `--force`** for `rm` and for overwriting via `insert`/`gen`.
5. **Read with `show`**: take `value` (the token), a field via `-f <key>`, or `fields.<key>`; binary secrets arrive as `base64`.
6. **Inject into subprocesses** with `run` instead of materialising secrets: `ks run -e db/url=DATABASE_URL -- ./migrate`.
7. **`edit` and `passwd` are unavailable in `--json`** (interactive only) — they return `{"error": "…"}`.
