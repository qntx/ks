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

test:
    cargo test --workspace --all-features

deny:
    cargo deny check

doc:
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features

gate: toml md fmt lint test deny doc
