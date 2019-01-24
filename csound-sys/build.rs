#[cfg(feature = "dynamic")]
extern crate pkg_config;


use std::env;
use std::env::consts;
use std::path::Path;
//use std::process::Command;

fn main(){
    if let Some(lib_dir) = env::var_os("CSOUND_LIB_DIR"){
        let lib_dir = Path::new(&lib_dir);
        let dylib_name = format!("{}csound64{}", consts::DLL_PREFIX, consts::DLL_SUFFIX);

        if lib_dir.join(dylib_name).exists(){
            println!("cargo:rustc-link-search=native={}", lib_dir.display());
            println!("cargo:rustc-link-lib=csound64");
            return;
        }else{
            println!("cargo:warning=library not found in ({}) trying with pkg-config",lib_dir.display());
        }
    }
    match pkg_config::probe_library("csound64"){
        Ok(info) => {
            for path in info.include_paths{
                println!("cargo:include={}", path.display());
            }
            return;
        },
        Err(err) => {
            println!("cargo:warning=pkg_config failed ({})", err);
            println!("If csound is already installed in your system,
            please help to pkg-config exporting the environment variable ");
            println!("CSOUND_LIB_DIR with the path to the csound library, for example ");
            println!("export CSOUND_LIB_DIR = /usr/lib  ");
            panic!();
        }
    }
}
