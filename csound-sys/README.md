[![Rust CI](https://github.com/neithanmo/csound-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/neithanmo/csound-rs/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/csound-sys.svg)](https://crates.io/crates/csound-sys)
[![docs.rs](https://docs.rs/csound-sys/badge.svg)](https://docs.rs/csound-sys)

# csound-sys

Native FFI bindings to the Csound 7 `libcsound64` library.

These are the low-level, auto-generated (bindgen) bindings. For safe, idiomatic
Rust bindings use the [`csound`](https://crates.io/crates/csound) crate.

The build script discovers a Csound 7 development installation via pkg-config,
the `CSOUND_INCLUDE_DIR` / `CSOUND_LIB_DIR` environment variables, or well-known
per-platform locations. See the [workspace README](../README.md) for details.
