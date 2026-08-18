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

# Check the callback trampolines' pointer handling under Miri.
#
# Miri refuses to call foreign functions, so it cannot execute a trampoline
# (each one starts with csoundGetHostData) or any test that builds a Csound
# instance. It can execute the unsafe core those trampolines delegate to, which
# is where the undefined behaviour would be.
miri:
    cargo +nightly miri test --lib trampoline_ptr

# Run the test suite under AddressSanitizer.
#
# This covers what Miri cannot: the trampolines actually running, with real
# Csound calling into them. Doctests are excluded because they do not link
# under -Zbuild-std with a sanitizer enabled.
asan:
    RUSTFLAGS="-Zsanitizer=address" \
    cargo +nightly test -Zbuild-std \
        --target $(rustc -vV | sed -n 's|host: ||p') \
        --workspace --tests
