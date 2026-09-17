#[allow(dead_code)]
#[path = "../resolve.rs"]
mod resolve;

use resolve::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn scratch(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("csound-rs-resolve-{name}-{nanos}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_headers(dir: &Path, major: u32) {
    fs::write(dir.join("csound.h"), "/* stub */\n").unwrap();
    fs::write(
        dir.join("version.h"),
        format!("#define CS_VERSION          ({major})\n#define CS_PATCHLEVEL       (0)\n"),
    )
    .unwrap();
}

#[test]
fn parses_cs_version_from_csound_header_style() {
    let text = "#define CS_VERSION          (7)\n#define CS_PATCHLEVEL       (0)\n";
    assert_eq!(csound_major_version_from_contents(text), Some(7));
    assert_eq!(
        csound_major_version_from_contents("#define CS_VERSION 6\n"),
        Some(6)
    );
}

#[test]
fn explicit_dirs_win_when_both_are_complete_csound7() {
    let root = scratch("ok");
    let include = root.join("include");
    let lib = root.join("lib");
    fs::create_dir_all(&include).unwrap();
    fs::create_dir_all(&lib).unwrap();
    write_headers(&include, 7);
    fs::write(lib.join("csound64.lib"), b"").unwrap();

    let resolved = explicit_csound7_dirs(Some(&include), Some(&lib), windows_library_ok);
    assert_eq!(resolved, Some((include, lib)));
}

#[test]
fn explicit_dirs_reject_csound6_headers() {
    let root = scratch("cs6");
    let include = root.join("include");
    let lib = root.join("lib");
    fs::create_dir_all(&include).unwrap();
    fs::create_dir_all(&lib).unwrap();
    write_headers(&include, 6);
    fs::write(lib.join("csound64.lib"), b"").unwrap();

    assert_eq!(
        explicit_csound7_dirs(Some(&include), Some(&lib), windows_library_ok),
        None
    );
}

#[test]
fn explicit_dirs_none_if_either_env_is_missing() {
    let root = scratch("partial");
    let include = root.join("include");
    fs::create_dir_all(&include).unwrap();
    write_headers(&include, 7);

    assert_eq!(explicit_csound7_dirs(Some(&include), None, |_| true), None);
    assert_eq!(explicit_csound7_dirs(None, Some(&root), |_| true), None);
}

#[test]
fn macos_lib_dir_may_be_the_folder_containing_the_framework() {
    let root = scratch("mac-parent");
    let framework = root.join(MACOS_FRAMEWORK_NAME);
    fs::create_dir_all(framework.join("Versions/7.0")).unwrap();
    fs::write(framework.join("Versions/7.0/CsoundLib64"), b"").unwrap();
    assert!(macos_library_ok(&root));
    assert_eq!(macos_framework_from_lib_dir(&root), framework);
}

#[test]
fn macos_lib_dir_may_be_the_framework_bundle_itself() {
    let root = scratch("mac-bundle");
    let framework = root.join(MACOS_FRAMEWORK_NAME);
    fs::create_dir_all(&framework).unwrap();
    fs::write(framework.join("CsoundLib64"), b"").unwrap();
    assert!(macos_library_ok(&framework));
    assert_eq!(macos_framework_from_lib_dir(&framework), framework);
}

#[test]
fn linux_library_ok_looks_for_the_host_dylib_name() {
    let lib = scratch("linux-lib");
    let name = format!(
        "{}csound64{}",
        std::env::consts::DLL_PREFIX,
        std::env::consts::DLL_SUFFIX
    );
    assert!(!linux_library_ok(&lib));
    fs::write(lib.join(name), b"").unwrap();
    assert!(linux_library_ok(&lib));
}

#[test]
fn try_explicit_env_none_if_only_one_dir_is_set() {
    let root = scratch("one-env");
    write_headers(&root, 7);
    assert_eq!(try_explicit_csound7_env(Some(&root), None, |_| true), None);
    assert_eq!(try_explicit_csound7_env(None, Some(&root), |_| true), None);
}

#[test]
#[should_panic(expected = "not a complete Csound 7 header set")]
fn try_explicit_env_panics_when_both_set_but_headers_are_csound6() {
    let root = scratch("env-cs6");
    let include = root.join("include");
    let lib = root.join("lib");
    fs::create_dir_all(&include).unwrap();
    fs::create_dir_all(&lib).unwrap();
    write_headers(&include, 6);
    fs::write(lib.join("csound64.lib"), b"").unwrap();
    let _ = try_explicit_csound7_env(Some(&include), Some(&lib), windows_library_ok);
}
