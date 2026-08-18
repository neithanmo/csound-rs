use std::env;
use std::path::{Path, PathBuf};

use bindgen::{EnumVariation, builder};

fn main() {
    println!("cargo:rustc-check-cfg=cfg(csound_sys_use_double)");
    if !link() {
        println!("cargo:warning=libcsound64 library not found in your system");
        println!(
            "export the CSOUND_LIB_DIR env var with the path to the csound library, for example "
        );
        println!("export CSOUND_LIB_DIR=/usr/lib  ");
        panic!();
    }

    generate_bindings();
}

fn generate_bindings() {
    println!("cargo:rerun-if-changed=csound/include/csound.h");
    println!("cargo:rerun-if-changed=csound/include/csdebug.h");
    println!("cargo:rerun-if-changed=csound/include/csound_circular_buffer.h");
    println!("cargo:rerun-if-changed=csound/include/csound_compiler.h");
    println!("cargo:rerun-if-changed=csound/include/csound_data_structures.h");
    println!("cargo:rerun-if-changed=csound/include/csound_files.h");
    println!("cargo:rerun-if-changed=csound/include/csound_graph_display.h");
    println!("cargo:rerun-if-changed=csound/include/csound_misc.h");
    println!("cargo:rerun-if-changed=csound/include/csound_rtaudio.h");
    println!("cargo:rerun-if-changed=csound/include/csound_rtmidi.h");
    println!("cargo:rerun-if-changed=csound/include/csound_server.h");
    println!("cargo:rerun-if-changed=csound/include/csound_threads.h");
    println!("cargo:rerun-if-changed=csound/include/csound_type_system.h");

    // mind there could be platform-dependent flags, so check compilation instructions per platform
    println!("cargo:rerun-if-env-changed=CSOUND_USE_DOUBLE");
    let use_double = match env::var("CSOUND_USE_DOUBLE") {
        Ok(val) => val != "0",
        Err(_) => true,
    };

    if use_double {
        println!("cargo:rustc-cfg=csound_sys_use_double");
    }

    let bindings = builder()
        .header("csound/include/csound.h")
        .header("csound/include/csdebug.h")
        .header("csound/include/csound_circular_buffer.h")
        .header("csound/include/csound_compiler.h")
        .header("csound/include/csound_data_structures.h")
        .header("csound/include/csound_files.h")
        .header("csound/include/csound_graph_display.h")
        .header("csound/include/csound_misc.h")
        .header("csound/include/csound_rtaudio.h")
        .header("csound/include/csound_rtmidi.h")
        .header("csound/include/csound_server.h")
        .header("csound/include/csound_threads.h")
        .header("csound/include/csound_type_system.h")
        .use_core()
        .default_enum_style(EnumVariation::ModuleConsts)
        .ctypes_prefix("libc")
        .derive_default(true)
        .derive_debug(true)
        // filter out all functions not starting by csound:
        .blocklist_function("__.*")
        .blocklist_function("[^c].*")
        .blocklist_function("c[^s].*")
        .blocklist_function("cs[^o].*")
        // default flags defined in CMakeLists (only those, which applicable)
        .clang_arg("-DUSE_LRINT");

    let bindings = if use_double {
        bindings.clang_arg("-DUSE_DOUBLE")
    } else {
        bindings
    }
    .generate()
    .expect("Unable generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}

#[cfg(target_os = "linux")]
fn link() -> bool {
    use std::env::consts;

    let dylib_name = format!("{}csound64{}", consts::DLL_PREFIX, consts::DLL_SUFFIX);

    if check_custom_path(&dylib_name) {
        return true;
    }

    // possible paths to find this library
    let paths = vec![Path::new("/usr/lib"), Path::new("/usr/local/lib")];
    for path in paths.as_slice() {
        if path.join(&dylib_name).exists() {
            println!("cargo:rustc-link-search=native={}", path.display());
            link_cmd(None);
            return true;
        }
    }

    false
}

#[cfg(target_os = "windows")]
fn link() -> bool {
    check_custom_path("csound64.lib")
}

#[cfg(target_os = "macos")]
fn link() -> bool {
    let framework = "CsoundLib64.framework";

    if check_custom_path(framework) {
        return true;
    }

    let system_dir = Path::new("/Library/Frameworks");

    if !system_dir.join(framework).exists() {
        return false;
    }

    link_cmd(Some(system_dir));

    true
}

fn check_custom_path(name: &str) -> bool {
    if let Some(lib_dir) = env::var_os("CSOUND_LIB_DIR") {
        let lib_dir = Path::new(&lib_dir);

        if !lib_dir.join(name).exists() {
            return false;
        }

        if cfg!(target_os = "linux") || cfg!(target_os = "windows") {
            println!("cargo:rustc-link-search=native={}", lib_dir.display());
            link_cmd(None);
        } else if cfg!(target_os = "macos") {
            link_cmd(Some(lib_dir));
        } else {
            unimplemented!()
        }

        return true;
    }

    false
}

/// Emits the link directives for the resolved Csound installation.
///
/// `framework_dir` is the directory *containing* `CsoundLib64.framework` and is
/// only meaningful on macOS; other platforms pass `None`.
fn link_cmd(framework_dir: Option<&Path>) {
    if cfg!(target_os = "linux") || cfg!(target_os = "windows") {
        println!("cargo:rustc-link-lib=csound64");
    } else if cfg!(target_os = "macos") {
        // Csound 7 records an @rpath-relative install name for the framework
        // (@rpath/CsoundLib64.framework/Versions/7.0/CsoundLib64). Without a
        // matching LC_RPATH on the consuming binary dyld cannot resolve it at
        // load time, so emit the rpath alongside the search path.
        //
        // Only the resolved directory is searched: adding /Library/Frameworks
        // unconditionally risks linking a system-wide Csound 6 over the
        // installation the user selected via CSOUND_LIB_DIR.
        if let Some(dir) = framework_dir {
            println!("cargo:rustc-link-search=framework={}", dir.display());
            println!("cargo:rustc-link-arg=-Wl,-rpath,{}", dir.display());
            // rustc-link-arg does not propagate across the dependency boundary,
            // so publish the directory through the `links = "csound64"` metadata
            // channel. Dependents see it as DEP_CSOUND64_FRAMEWORK_DIR and
            // re-emit the rpath for their own binaries (see ../build.rs).
            println!("cargo:framework_dir={}", dir.display());
        }
        println!("cargo:rustc-link-lib=framework=CsoundLib64");
    } else {
        unimplemented!()
    }
}
