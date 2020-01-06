use std::env;
use std::env::consts;
use std::path::Path;

fn main() {
    if let Some(lib_dir) = env::var_os("CSOUND_LIB_DIR") {
        let lib_dir = Path::new(&lib_dir);
        let dylib_name;
        let target = env::var("TARGET").unwrap();
        let windows = target.contains("windows");
        if windows {
            dylib_name = "csound64.lib".to_owned();
        } else {
            dylib_name = format!("{}csound64{}", consts::DLL_PREFIX, consts::DLL_SUFFIX);
        }
        if lib_dir.join(dylib_name).exists() {
            println!("cargo:rustc-link-search=native={}", lib_dir.display());
            println!("cargo:rustc-link-lib=csound64");
            return;
        } else {
            println!("cargo:warning=library not found in {}", lib_dir.display());
            println!("export CSOUND_LIB_DIR with the path to the csound library, for example ");
            println!("export CSOUND_LIB_DIR=/usr/lib  ");
            panic!();
        }
    }
}
