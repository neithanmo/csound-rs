use std::env;

fn main() {
    // csound-sys records a runtime library search path (rpath) in binaries
    // wherever the dynamic loader could not otherwise find Csound:
    //
    // - On macOS the Csound 7 framework records an @rpath-relative install name
    //   (@rpath/CsoundLib64.framework/Versions/7.0/CsoundLib64). A binary
    //   linking it needs a matching LC_RPATH or dyld fails at load time with
    //   "no LC_RPATH's found".
    // - On Linux a library selected with CSOUND_LIB_DIR is usually outside the
    //   loader's search path, so it would not be found, or another Csound 7
    //   from the loader cache would be loaded instead.
    //
    // A build script's `rustc-link-arg` only applies to its own package's
    // targets. csound-sys therefore republishes the directory via its
    // `links = "csound64"` metadata, which Cargo hands to us as
    // DEP_CSOUND64_RPATH. Re-emitting the rpath here covers this crate's tests,
    // examples and benches. The variable is unset when no rpath is needed.
    //
    // Downstream crates that build an executable against `csound` need the same
    // lines in their own build script.
    println!("cargo:rerun-if-changed=build.rs");

    if let Ok(dir) = env::var("DEP_CSOUND64_RPATH") {
        println!("cargo:rustc-link-arg=-Wl,-rpath,{dir}");
    }
}
