# Local gate recipes; `just gate` runs the full pre-PR gate.

default: gate

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

gate: fmt lint test deny doc
