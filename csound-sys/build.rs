use std::env;
use std::path::{Path, PathBuf};

use bindgen::{EnumVariation, builder};

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
    generate_bindings(&include_dir);
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
    use std::env::consts;

    println!("cargo:rerun-if-env-changed=CSOUND_INCLUDE_DIR");
    println!("cargo:rerun-if-env-changed=CSOUND_LIB_DIR");

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
    let dylib_name = format!("{}csound64{}", consts::DLL_PREFIX, consts::DLL_SUFFIX);
    let mut installations = vec![
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

    // Explicit paths are the final fallback for custom Linux installations.
    // Requiring both preserves the matched header/library pair.
    if let (Some(include_dir), Some(library_dir)) = (
        env::var_os("CSOUND_INCLUDE_DIR"),
        env::var_os("CSOUND_LIB_DIR"),
    ) {
        installations.push((include_dir.into(), library_dir.into()));
    }

    let (include_dir, library_dir) = installations
        .into_iter()
        .find(|(include_dir, library_dir)| {
            include_dir.join("csound.h").is_file()
                && csound_major_version(include_dir).is_some_and(|major| major >= 7)
                && library_dir.join(&dylib_name).is_file()
        })
        .unwrap_or_else(|| {
            panic!(
                "Could not find a complete Csound 7 development installation. pkg-config \
                 failed: {pkg_config_error}. Install the Csound development files, or set both \
                 CSOUND_INCLUDE_DIR (the directory containing csound.h) and CSOUND_LIB_DIR (the \
                 directory containing {dylib_name})."
            )
        });

    println!("cargo:rustc-link-search=native={}", library_dir.display());
    link_cmd(None);

    include_dir
}

fn csound_major_version(include_dir: &Path) -> Option<u32> {
    let contents = std::fs::read_to_string(include_dir.join("version.h")).ok()?;
    let definition = contents
        .lines()
        .find(|line| line.trim_start().starts_with("#define CS_VERSION"))?;

    definition
        .split(|character: char| !character.is_ascii_digit())
        .find(|part| !part.is_empty())?
        .parse()
        .ok()
}

#[cfg(target_os = "windows")]
fn setup_csound() -> PathBuf {
    println!("cargo:rerun-if-env-changed=CSOUND_INCLUDE_DIR");
    println!("cargo:rerun-if-env-changed=CSOUND_LIB_DIR");

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

    // Explicit paths are the final fallback for custom installations. Require
    // both so headers and the import library cannot come from different builds.
    if let (Some(include_dir), Some(library_dir)) = (
        env::var_os("CSOUND_INCLUDE_DIR"),
        env::var_os("CSOUND_LIB_DIR"),
    ) {
        installations.push((include_dir.into(), library_dir.into()));
    }

    let (include_dir, library_dir) = installations
        .into_iter()
        .find(|(include_dir, library_dir)| {
            include_dir.join("csound.h").is_file()
                && csound_major_version(include_dir).is_some_and(|major| major >= 7)
                && library_dir.join("csound64.lib").is_file()
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
    const FRAMEWORK_NAME: &str = "CsoundLib64.framework";

    println!("cargo:rerun-if-env-changed=CSOUND_INCLUDE_DIR");
    println!("cargo:rerun-if-env-changed=CSOUND_LIB_DIR");

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
        let framework = framework_dir.join(FRAMEWORK_NAME);
        if !macos_framework_binary_exists(&framework) {
            continue;
        }

        for include_dir in [
            framework.join("Versions/7.0/Headers"),
            framework.join("Headers"),
        ] {
            if include_dir.join("csound.h").is_file()
                && csound_major_version(&include_dir).is_some_and(|major| major >= 7)
            {
                link_cmd(Some(&framework_dir));
                return include_dir;
            }
        }
    }

    // Explicit paths are the final fallback. CSOUND_LIB_DIR may be either the
    // directory containing the framework or the framework bundle itself.
    if let (Some(include_dir), Some(library_dir)) = (
        env::var_os("CSOUND_INCLUDE_DIR").map(PathBuf::from),
        env::var_os("CSOUND_LIB_DIR").map(PathBuf::from),
    ) {
        let framework = if library_dir
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == FRAMEWORK_NAME)
        {
            library_dir.clone()
        } else {
            library_dir.join(FRAMEWORK_NAME)
        };

        if include_dir.join("csound.h").is_file()
            && csound_major_version(&include_dir).is_some_and(|major| major >= 7)
            && macos_framework_binary_exists(&framework)
        {
            let framework_dir = framework
                .parent()
                .expect("Csound framework must have a parent directory");
            link_cmd(Some(framework_dir));
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

#[cfg(target_os = "macos")]
fn macos_framework_binary_exists(framework: &Path) -> bool {
    framework.join("CsoundLib64").is_file() || framework.join("Versions/7.0/CsoundLib64").is_file()
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
