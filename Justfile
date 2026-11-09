# Local gate recipes; `just gate` runs the full pre-PR gate.

default: gate

toml:
    npx --yes @taplo/cli@0.7.0 fmt --check

md:
    npx --yes markdownlint-cli2@0.23.3 "*.md" "skills/**/*.md"

fmt:
    cargo fmt --all --check

lint:
    cargo clippy --workspace --all-targets --all-features -- -D warnings
    cargo clippy -p ks --all-targets --no-default-features -- -D warnings

# Cross-target lint catches cfg(unix)/cfg(windows) code the host never compiles
# (needs `rustup target add x86_64-unknown-linux-gnu x86_64-pc-windows-msvc`).
lint-cross:
    # ks-desktop (Tauri) cannot be cross-checked from macOS.
    cargo clippy -p ks -p ks-clipboard -p ks-cli --all-targets --all-features --target x86_64-unknown-linux-gnu -- -D warnings
    cargo clippy -p ks -p ks-clipboard -p ks-cli --all-targets --all-features --target x86_64-pc-windows-msvc -- -D warnings

test:
    cargo test --workspace --all-features

deny:
    cargo deny check

doc:
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features

gate: toml md fmt lint lint-cross test deny doc js-gate

js-gate:
    bun run lint
    bun run typecheck
    bun run build
    bun run test
