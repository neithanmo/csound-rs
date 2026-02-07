# Justfile for csound-rs

# Build in debug mode
build:
    cargo build --workspace

# Build all examples in release mode
examples:
    cargo build --examples --release

# Format code
fmt:
    cargo +nightly fmt --all

# Run clippy with warnings as errors (same as CI)
clippy:
    cargo +nightly clippy --workspace --all-targets -- -D warnings

# Run tests
tests:
    cargo test --workspace

# Run ignored tests
tests-ignored:
    cargo test --workspace -- --ignored
