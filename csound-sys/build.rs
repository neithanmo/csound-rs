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
        // black list
        .blacklist_function("__fpclassifyl")
        .blacklist_function("__infl")
        .blacklist_function("acoshl")
        .blacklist_function("acosl")
        .blacklist_function("asinhl")
        .blacklist_function("asinl")
        .blacklist_function("atan2l")
        .blacklist_function("atanhl")
        .blacklist_function("atanl")
        .blacklist_function("cbrtl")
        .blacklist_function("ceill")
        .blacklist_function("copysignl")
        .blacklist_function("coshl")
        .blacklist_function("cosl")
        .blacklist_function("erfcl")
        .blacklist_function("erfl")
        .blacklist_function("exp2l")
        .blacklist_function("expl")
        .blacklist_function("expm1l")
        .blacklist_function("fabsl")
        .blacklist_function("fdiml")
        .blacklist_function("floorl")
        .blacklist_function("fmal")
        .blacklist_function("fmaxl")
        .blacklist_function("fminl")
        .blacklist_function("fmodl")
        .blacklist_function("frexpl")
        .blacklist_function("hypotl")
        .blacklist_function("ilogbl")
        .blacklist_function("ldexpl")
        .blacklist_function("lgammal")
        .blacklist_function("llrintl")
        .blacklist_function("llroundl")
        .blacklist_function("log10l")
        .blacklist_function("log1pl")
        .blacklist_function("log2l")
        .blacklist_function("logbl")
        .blacklist_function("logl")
        .blacklist_function("lrintl")
        .blacklist_function("lroundl")
        .blacklist_function("modfl")
        .blacklist_function("nanl")
        .blacklist_function("nearbyintl")
        .blacklist_function("nextafterl")
        .blacklist_function("nexttoward")
        .blacklist_function("nexttowardf")
        .blacklist_function("nexttowardl")
        .blacklist_function("powl")
        .blacklist_function("remainderl")
        .blacklist_function("remquol")
        .blacklist_function("rintl")
        .blacklist_function("roundl")
        .blacklist_function("scalblnl")
        .blacklist_function("scalbnl")
        .blacklist_function("sinhl")
        .blacklist_function("sinl")
        .blacklist_function("sqrtl")
        .blacklist_function("strtold")
        .blacklist_function("tanhl")
        .blacklist_function("tanl")
        .blacklist_function("tgammal")
        .blacklist_function("truncl")
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
