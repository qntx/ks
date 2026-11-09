# AGENTS.md

- Do not preserve backward compatibility. Remove obsolete paths instead of adding compatibility layers, fallbacks, or migrations.
- Choose the simplest implementation that fully meets the current requirements. Avoid speculative abstractions, configuration, and indirection.
- Grow the system in layers. Start from the smallest version that works end to end, and add each new capability on top of a product that already works. Never trade a working product for unfinished complexity.
- Keep components modular and concerns clearly separated.
- Prefer established, well-maintained libraries when they reduce overall complexity or improve reliability. Do not reimplement common functionality without a clear reason.
- Lean on the dependencies already in the project before writing your own implementation or adding packages. Do not assume a library lacks a capability without checking its documentation and types.
- Make architectural decisions for the long term. Do not accept a stopgap that only works for now and is meant to be replaced later.

## Architecture invariants

- `ks` is rewritten from scratch for 0.8.0; the 0.7 code and formats are gone
  without migration. `design/` is local-only and never committed.
- The `ks` library is `#![forbid(unsafe_code)]`. `ks-cli` may use `unsafe` only
  in `harden.rs`; `ks-clipboard` only inside its platform module (`macos.rs`)
  in `harden.rs` and in the agent peer-credential check, each with a `SAFETY`
  comment. `ks-cli` only uses the library's public API.
- Secret material lives in `Zeroizing`/`age::secrecy` types and is redacted
  from `Debug`. The vault recipient `R` is never written to disk, printed, or
  sent over the agent protocol.
- Every vault entry file has exactly one `X25519` recipient stanza; production
  code does not panic — no `unwrap`, `expect`, indexing panics, or `panic!`
  outside tests.
- Dependencies are declared once in `[workspace.dependencies]`; new or updated
  dependencies must be released for at least 7 days
  and `signer`, which are pinned with `=` requirements.

## Commands

Local gate before a pull request (`just gate` runs all of it):

```bash
npx --yes @taplo/cli@0.7.0 fmt --check
npx --yes markdownlint-cli2@0.23.3 "*.md" "skills/**/*.md"
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo clippy -p ks --all-targets --no-default-features -- -D warnings
cargo clippy -p ks -p ks-clipboard -p ks-cli --all-targets --all-features --target x86_64-unknown-linux-gnu -- -D warnings
cargo clippy -p ks -p ks-clipboard -p ks-cli --all-targets --all-features --target x86_64-pc-windows-msvc -- -D warnings
cargo test --workspace --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features
cargo deny check
bun run lint && bun run typecheck && bun run build && bun run test
```

`vp run` caches task results locally (`run.cache` is off only when `CI` is
set); prefix the JS gate with `CI=1` for an uncached run before a pull
request.

Run the JS gate from the repository root: `bun run lint` there runs
`vp check`, taplo and the desktop style guard together. `ks-desktop` cannot
be cross-checked from macOS (Tauri needs the GTK sysroot), so CI on Linux is
the first build of its `cfg(not(target_os = "macos"))` branches; keep each
platform pair small, and make sure items used only on one side are gated
to it.
