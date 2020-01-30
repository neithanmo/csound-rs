use std::env;
use std::env::consts;
use std::path::Path;

fn main() {

    let dylib_name;
    let target = env::var("TARGET").unwrap();

    if !target.contains("windows") {
        
        // csound lib name on unix
        dylib_name = format!("{}csound64{}", consts::DLL_PREFIX, consts::DLL_SUFFIX);
        let mut paths = Vec::new();
        
        // posible paths to find this library
        paths.push(Path::new("/usr/lib"));
        paths.push(Path::new("/usr/local/lib"));
        
        for path in paths.as_slice() {
            if path.join(&dylib_name).exists() {
                println!("cargo:rustc-link-search=native={}", path.display());
                println!("cargo:rustc-link-lib=csound64");
                return;
            }
        }
    } else {
        // windows case
        dylib_name = "csound64.lib".to_owned();
    }
     

    if let Some(lib_dir) = env::var_os("CSOUND_LIB_DIR") {
        let lib_dir = Path::new(&lib_dir);
        if lib_dir.join(dylib_name).exists() {
            println!("cargo:rustc-link-search=native={}", lib_dir.display());
            println!("cargo:rustc-link-lib=csound64");
            return;
        }
    }
    println!("cargo:warning=libcsound64 library not found in your system");
    println!("export the CSOUND_LIB_DIR env var with the path to the csound library, for example ");
    println!("export CSOUND_LIB_DIR=/usr/lib  ");
    panic!();
}
