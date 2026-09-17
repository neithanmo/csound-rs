use std::env;
use std::path::{Path, PathBuf};

use bindgen::{EnumVariation, builder};

#[allow(dead_code)]
#[path = "resolve.rs"]
mod resolve;
use resolve::{check_include_dir, env_path, explicit_csound7_dirs};

// Bindgen discovers headers included by these files, but it cannot know which
// standalone Csound headers are part of the API we intend to expose. Keep that
// root set explicit; CargoCallbacks tracks all of their transitive includes.
const CSOUND_HEADERS: &[&str] = &[
    "csound.h",
    "csdebug.h",
    "csound_circular_buffer.h",
    "csound_compiler.h",
    "csound_data_structures.h",
    "csound_files.h",
    "csound_graph_display.h",
    "csound_misc.h",
    "csound_rtaudio.h",
    "csound_rtmidi.h",
    "csound_server.h",
    "csound_threads.h",
    "csound_type_system.h",
];

fn main() {
    println!("cargo:rustc-check-cfg=cfg(csound_sys_use_double)");

    let include_dir = setup_csound();
    compile_shim(&include_dir);
    generate_bindings(&include_dir);
}

fn compile_shim(include_dir: &Path) {
    let csdl_header = include_dir.join("csdl.h");
    if !csdl_header.is_file() {
        panic!(
            "The Csound development installation at '{}' is incomplete: csdl.h is required to \
             build the temporary control-channel-hints deallocation shim. Install the complete \
             Csound 7 development/plugin headers, or point CSOUND_INCLUDE_DIR and CSOUND_LIB_DIR \
             to a matching complete Csound installation.",
            include_dir.display()
        );
    }

    println!("cargo:rerun-if-changed=src/csound_shim.c");

    let mut build = cc::Build::new();
    build.file("src/csound_shim.c").include(include_dir);
    if env::var("CSOUND_USE_DOUBLE").map_or(true, |value| value != "0") {
        build.define("USE_DOUBLE", None);
    }
    build.compile("csound_rs_shim");
}

/// Returns the installation named by `CSOUND_INCLUDE_DIR` and `CSOUND_LIB_DIR`,
/// or `None` when neither is set. Fails the build if only one is set or the
/// pair is not a usable Csound 7 installation.
fn explicit_csound_dirs(
    check_library: impl Fn(&Path) -> Result<(), String>,
) -> Option<(PathBuf, PathBuf)> {
    println!("cargo:rerun-if-env-changed=CSOUND_INCLUDE_DIR");
    println!("cargo:rerun-if-env-changed=CSOUND_LIB_DIR");
    explicit_csound7_dirs(
        env_path(env::var_os("CSOUND_INCLUDE_DIR")),
        env_path(env::var_os("CSOUND_LIB_DIR")),
        check_library,
    )
    .unwrap_or_else(|error| panic!("{error}"))
}

fn generate_bindings(include_dir: &Path) {
    // mind there could be platform-dependent flags, so check compilation instructions per platform
    println!("cargo:rerun-if-env-changed=CSOUND_USE_DOUBLE");
    let use_double = match env::var("CSOUND_USE_DOUBLE") {
        Ok(val) => val != "0",
        Err(_) => true,
    };

    if use_double {
        println!("cargo:rustc-cfg=csound_sys_use_double");
    }

    let mut bindings = builder();
    for header in CSOUND_HEADERS {
        bindings = bindings.header(include_dir.join(header).to_string_lossy());
    }

    let bindings = bindings
        .clang_arg(format!("-I{}", include_dir.display()))
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
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
        // Provided by our ownership shim until Csound exposes this host API.
        .blocklist_function("csoundFreeControlChannelHints")
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
fn setup_csound() -> PathBuf {
    use resolve::{LINUX_LIBRARY_NAME, check_linux_library};

    if let Some((include_dir, library_dir)) = explicit_csound_dirs(check_linux_library) {
        println!("cargo:rustc-link-search=native={}", library_dir.display());
        // A custom directory is normally not on the dynamic loader's search
        // path, and `cargo run`/`cargo test` only add link-search paths inside
        // `target/` to it. Without an rpath, binaries would fail to start or
        // load another Csound 7 from the loader cache.
        emit_rpath(&library_dir);
        link_cmd(None);
        return include_dir;
    }

    let pkg_config_error = match pkg_config::Config::new()
        .atleast_version("7.0")
        .cargo_metadata(true)
        .probe("csound")
    {
        Ok(library) => {
            if let Some(include_dir) = library
                .include_paths
                .into_iter()
                .find(|path| path.join("csound.h").is_file())
            {
                return include_dir;
            }
            "pkg-config found Csound 7, but its include paths do not contain csound.h".to_owned()
        }
        Err(error) => error.to_string(),
    };

    // Csound's default source-install prefix is /usr/local, while distro
    // packages normally install under /usr. Keep each include/library pair
    // together so bindings cannot accidentally be generated for one install
    // and linked against another.
    let installations = [
        (
            PathBuf::from("/usr/local/include/csound"),
            PathBuf::from("/usr/local/lib"),
        ),
        (
            PathBuf::from("/usr/include/csound"),
            PathBuf::from("/usr/lib"),
        ),
        (PathBuf::from("/usr/include"), PathBuf::from("/usr/lib")),
    ];

    let (include_dir, library_dir) = installations
        .into_iter()
        .find(|(include_dir, library_dir)| {
            check_include_dir(include_dir).is_ok() && check_linux_library(library_dir).is_ok()
        })
        .unwrap_or_else(|| {
            panic!(
                "Could not find a complete Csound 7 development installation. pkg-config \
                 failed: {pkg_config_error}. Install the Csound development files, or set both \
                 CSOUND_INCLUDE_DIR (the directory containing csound.h) and CSOUND_LIB_DIR (the \
                 directory containing {LINUX_LIBRARY_NAME})."
            )
        });

    println!("cargo:rustc-link-search=native={}", library_dir.display());
    link_cmd(None);

    include_dir
}

#[cfg(target_os = "windows")]
fn setup_csound() -> PathBuf {
    use resolve::check_windows_library;

    if let Some((include_dir, library_dir)) = explicit_csound_dirs(check_windows_library) {
        println!("cargo:rustc-link-search=native={}", library_dir.display());
        link_cmd(None);
        return include_dir;
    }

    let program_files = env::var_os("ProgramFiles")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Program Files"));
    let mut installations = Vec::new();

    // Csound 7 installers normally use C:\Program Files\Csound. Also inspect
    // versioned x64 installation directories. A real Csound 6 installation is
    // still rejected below by checking CS_VERSION in version.h.
    for root in [
        program_files.join("Csound"),
        program_files.join("Csound7_x64"),
        program_files.join("Csound6_x64"),
    ] {
        for include_dir in [root.join("include"), root.join("include/csound")] {
            for library_dir in [root.join("lib"), root.join("bin")] {
                installations.push((include_dir.clone(), library_dir));
            }
        }
    }

    let (include_dir, library_dir) = installations
        .into_iter()
        .find(|(include_dir, library_dir)| {
            check_include_dir(include_dir).is_ok() && check_windows_library(library_dir).is_ok()
        })
        .unwrap_or_else(|| {
            panic!(
                "Could not find a complete Csound 7 development installation. Install Csound 7 \
                 under C:\\Program Files\\Csound, or set both CSOUND_INCLUDE_DIR (the directory \
                 containing csound.h) and CSOUND_LIB_DIR (the directory containing csound64.lib)."
            )
        });

    println!("cargo:rustc-link-search=native={}", library_dir.display());
    link_cmd(None);

    include_dir
}

#[cfg(target_os = "macos")]
fn setup_csound() -> PathBuf {
    use resolve::{
        MACOS_FRAMEWORK_NAME, check_macos_library, macos_framework_from_lib_dir,
        macos_framework_version_dir,
    };

    if let Some((include_dir, library_dir)) = explicit_csound_dirs(check_macos_library) {
        let framework = macos_framework_from_lib_dir(&library_dir);
        let framework_dir = framework
            .parent()
            .expect("Csound framework must have a parent directory");
        link_cmd(Some(framework_dir));
        return include_dir;
    }

    let mut framework_dirs = vec![
        PathBuf::from("/Library/Frameworks"),
        PathBuf::from("/Applications/Csound"),
    ];
    if let Some(home) = env::var_os("HOME") {
        framework_dirs.push(PathBuf::from(home).join("Library/Frameworks"));
    }
    framework_dirs.extend([
        PathBuf::from("/opt/homebrew/Frameworks"),
        PathBuf::from("/opt/homebrew/lib"),
        PathBuf::from("/usr/local/Frameworks"),
        PathBuf::from("/usr/local/lib"),
        PathBuf::from("/opt/local/Library/Frameworks"),
        PathBuf::from("/opt/local/lib"),
    ]);

    for framework_dir in framework_dirs {
        let framework = framework_dir.join(MACOS_FRAMEWORK_NAME);
        // Take the headers that belong to the binary the linker will use, so a
        // Versions/7.0 left beside a Current -> 6.0 link is not mistaken for
        // a Csound 7 framework.
        let Some(version_dir) = macos_framework_version_dir(&framework) else {
            continue;
        };
        let include_dir = version_dir.join("Headers");
        if check_include_dir(&include_dir).is_ok() {
            link_cmd(Some(&framework_dir));
            return include_dir;
        }
    }

    panic!(
        "Could not find a complete Csound 7 framework installation. Install \
         CsoundLib64.framework under /Library/Frameworks or ~/Library/Frameworks, or set both \
         CSOUND_INCLUDE_DIR (the framework Headers directory) and CSOUND_LIB_DIR (the directory \
         containing CsoundLib64.framework)."
    );
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
            emit_rpath(dir);
            // The macOS-only name that emit_rpath's DEP_CSOUND64_RPATH replaces.
            // Kept so existing dependents' build scripts keep working.
            println!("cargo:framework_dir={}", dir.display());
        }
        println!("cargo:rustc-link-lib=framework=CsoundLib64");
    } else {
        unimplemented!()
    }
}

/// Records `dir` as a runtime library search path (rpath) in this package's
/// binaries.
///
/// `rustc-link-arg` does not propagate across the dependency boundary, so the
/// directory is also published through the `links = "csound64"` metadata
/// channel. Dependents see it as DEP_CSOUND64_RPATH and re-emit the rpath for
/// their own binaries (see ../build.rs).
fn emit_rpath(dir: &Path) {
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", dir.display());
    println!("cargo:rpath={}", dir.display());
}
