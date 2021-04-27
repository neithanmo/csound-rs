use std::env;
use std::path::{Path, PathBuf};

use bindgen::{builder, EnumVariation};

fn main() {
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

    // mind there could be platform-dependent flags, so check compilation instructions per platform
    let bindings = builder()
        .header("csound/include/csound.h")
        .use_core()
        .default_enum_style(EnumVariation::ModuleConsts)
        .ctypes_prefix("libc")
        .derive_default(true)
        .derive_debug(true)

        // filter out all functions not starting by csound:
        .blacklist_function("__.*")
        .blacklist_function("[^c].*")
        .blacklist_function("c[^s].*")
        .blacklist_function("cs[^o].*")

        // default flags defined in CMakeLists (only those, which applicable)
        .clang_arg("-DUSE_DOUBLE")
        .clang_arg("-DUSE_LRINT")
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

    let mut paths = Vec::new();
    // posible paths to find this library
    paths.push(Path::new("/usr/lib"));
    paths.push(Path::new("/usr/local/lib"));
    for path in paths.as_slice() {
        if path.join(&dylib_name).exists() {
            println!("cargo:rustc-link-search=native={}", path.display());
            link_cmd();
            return true;
        }
    }

    return false;
}

#[cfg(target_os = "windows")]
fn link() -> bool {
    return check_custom_path("csound64.lib");
}

#[cfg(target_os = "macos")]
fn link() -> bool {
    let framework = "CsoundLib64.framework";

    if check_custom_path(framework) {
        return true;
    }

    let path_str = format!("/Library/Frameworks/{}", framework);

    if !Path::new(&path_str).exists() {
        return false;
    }

    link_cmd();

    return true;
}

fn check_custom_path(name: &str) -> bool {
    if let Some(lib_dir) = env::var_os("CSOUND_LIB_DIR") {
        let lib_dir = Path::new(&lib_dir);

        if !lib_dir.join(name).exists() {
            return false;
        }

        if cfg!(linux) || cfg!(windows) {
            println!("cargo:rustc-link-search=native={}", lib_dir.display());
            link_cmd();
        } else if cfg!(macos) {
            println!("cargo:rustc-link-search=framework={}", lib_dir.display());
            link_cmd();
        } else {
            unimplemented!()
        }

        return true;
    }

    return false;
}

fn link_cmd() {
    if cfg!(target_os = "linux") || cfg!(target_os = "windows") {
        println!("cargo:rustc-link-lib=csound64");
    } else if cfg!(target_os = "macos") {
        println!("cargo:rustc-link-search=framework=/Library/Frameworks");
        println!("cargo:rustc-link-lib=framework=CsoundLib64");
    } else {
        unimplemented!()
    }
}
