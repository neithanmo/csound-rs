use std::env;

fn main() {
    // On macOS the Csound 7 framework records an @rpath-relative install name
    // (@rpath/CsoundLib64.framework/Versions/7.0/CsoundLib64). A binary linking
    // it needs a matching LC_RPATH or dyld fails at load time with
    // "no LC_RPATH's found".
    //
    // csound-sys resolves the framework location, but a build script's
    // `rustc-link-arg` only applies to its own package's targets. It therefore
    // republishes the directory via its `links = "csound64"` metadata, which
    // Cargo hands to us as DEP_CSOUND64_FRAMEWORK_DIR. Re-emitting the rpath
    // here covers this crate's tests, examples and benches.
    //
    // Downstream crates that build an executable against `csound` need the same
    // three lines in their own build script.
    println!("cargo:rerun-if-changed=build.rs");

    if cfg!(target_os = "macos")
        && let Ok(dir) = env::var("DEP_CSOUND64_FRAMEWORK_DIR")
    {
        println!("cargo:rustc-link-arg=-Wl,-rpath,{dir}");
    }
}
