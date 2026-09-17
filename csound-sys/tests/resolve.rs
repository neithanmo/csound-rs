#[allow(dead_code)]
#[path = "../resolve.rs"]
mod resolve;

use resolve::*;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// A temporary directory that is removed when the test ends.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "csound-rs-resolve-{name}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    /// Creates `relative` inside the scratch directory and returns its path.
    fn dir(&self, relative: &str) -> PathBuf {
        let dir = self.0.join(relative);
        fs::create_dir_all(&dir).unwrap();
        dir
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn write_headers(dir: &Path, major: u32) {
    fs::create_dir_all(dir).unwrap();
    fs::write(dir.join("csound.h"), "/* stub */\n").unwrap();
    fs::write(
        dir.join("version.h"),
        format!("#define CS_VERSION          ({major})\n#define CS_PATCHLEVEL       (0)\n"),
    )
    .unwrap();
}

/// Creates `Versions/<version>` with a binary and headers reporting `major`.
fn write_framework_version(framework: &Path, version: &str, major: u32) {
    let version_dir = framework.join("Versions").join(version);
    write_headers(&version_dir.join("Headers"), major);
    fs::write(version_dir.join("CsoundLib64"), b"").unwrap();
}

/// Points `Versions/Current` and the top-level binary link at `version`, as
/// an installed framework does.
#[cfg(unix)]
fn link_framework_current(framework: &Path, version: &str) {
    use std::os::unix::fs::symlink;

    let current = framework.join("Versions/Current");
    let _ = fs::remove_file(&current);
    symlink(version, &current).unwrap();
    let binary = framework.join("CsoundLib64");
    if fs::symlink_metadata(&binary).is_err() {
        symlink("Versions/Current/CsoundLib64", binary).unwrap();
    }
}

fn library_ok(_: &Path) -> Result<(), String> {
    Ok(())
}

fn library_missing(_: &Path) -> Result<(), String> {
    Err("stub library missing".to_owned())
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
fn empty_env_values_count_as_unset() {
    assert_eq!(env_path(None), None);
    assert_eq!(env_path(Some(OsString::new())), None);
    assert_eq!(
        env_path(Some(OsString::from("/opt/csound"))),
        Some(PathBuf::from("/opt/csound"))
    );
}

#[test]
fn explicit_dirs_are_not_used_when_neither_is_set() {
    assert_eq!(explicit_csound7_dirs(None, None, library_ok), Ok(None));
}

#[test]
fn explicit_dirs_reject_a_lone_include_dir() {
    let scratch = Scratch::new("lone-include");
    let include = scratch.dir("include");
    write_headers(&include, 7);

    let error = explicit_csound7_dirs(Some(include), None, library_ok).unwrap_err();
    assert!(
        error.starts_with("CSOUND_INCLUDE_DIR is set but CSOUND_LIB_DIR is not"),
        "{error}"
    );
}

#[test]
fn explicit_dirs_reject_a_lone_lib_dir() {
    let scratch = Scratch::new("lone-lib");
    let lib = scratch.dir("lib");

    let error = explicit_csound7_dirs(None, Some(lib), library_ok).unwrap_err();
    assert!(
        error.starts_with("CSOUND_LIB_DIR is set but CSOUND_INCLUDE_DIR is not"),
        "{error}"
    );
}

#[test]
fn explicit_dirs_accept_a_complete_csound7_pair() {
    let scratch = Scratch::new("ok");
    let include = scratch.dir("include");
    let lib = scratch.dir("lib");
    write_headers(&include, 7);
    fs::write(lib.join(WINDOWS_LIBRARY_NAME), b"").unwrap();

    let resolved = explicit_csound7_dirs(
        Some(include.clone()),
        Some(lib.clone()),
        check_windows_library,
    );
    assert_eq!(resolved, Ok(Some((include, lib))));
}

#[test]
fn explicit_dirs_reject_csound6_headers() {
    let scratch = Scratch::new("cs6");
    let include = scratch.dir("include");
    write_headers(&include, 6);

    let error =
        explicit_csound7_dirs(Some(include), Some(scratch.dir("lib")), library_ok).unwrap_err();
    assert!(error.starts_with("CSOUND_INCLUDE_DIR ("), "{error}");
    assert!(error.ends_with("version.h reports Csound 6"), "{error}");
}

#[test]
fn explicit_dirs_reject_an_include_dir_without_csound_h() {
    let scratch = Scratch::new("no-csound-h");

    let error = explicit_csound7_dirs(
        Some(scratch.dir("include")),
        Some(scratch.dir("lib")),
        library_ok,
    )
    .unwrap_err();
    assert!(error.ends_with("csound.h not found"), "{error}");
}

#[test]
fn explicit_dirs_report_why_the_library_dir_was_rejected() {
    let scratch = Scratch::new("bad-lib");
    let include = scratch.dir("include");
    write_headers(&include, 7);

    let error = explicit_csound7_dirs(Some(include), Some(scratch.dir("lib")), library_missing)
        .unwrap_err();
    assert!(error.starts_with("CSOUND_LIB_DIR ("), "{error}");
    assert!(error.ends_with("stub library missing"), "{error}");
}

#[test]
fn windows_library_requires_the_import_library() {
    let scratch = Scratch::new("windows-lib");
    let lib = scratch.dir("lib");
    assert_eq!(
        check_windows_library(&lib),
        Err("csound64.lib not found".to_owned())
    );
    fs::write(lib.join(WINDOWS_LIBRARY_NAME), b"").unwrap();
    assert_eq!(check_windows_library(&lib), Ok(()));
}

#[test]
fn linux_library_requires_libcsound64_so() {
    let scratch = Scratch::new("linux-missing");
    assert_eq!(
        check_linux_library(&scratch.dir("lib")),
        Err("libcsound64.so not found".to_owned())
    );
}

#[test]
fn linux_library_without_a_version_suffix_is_accepted() {
    let scratch = Scratch::new("linux-plain");
    let lib = scratch.dir("lib");
    fs::write(lib.join(LINUX_LIBRARY_NAME), b"").unwrap();
    assert_eq!(check_linux_library(&lib), Ok(()));
}

#[cfg(unix)]
#[test]
fn linux_library_follows_the_link_to_the_versioned_library() {
    use std::os::unix::fs::symlink;

    let scratch = Scratch::new("linux-soname");
    let csound7 = scratch.dir("csound7");
    fs::write(csound7.join("libcsound64.so.7.0"), b"").unwrap();
    symlink("libcsound64.so.7.0", csound7.join(LINUX_LIBRARY_NAME)).unwrap();
    assert_eq!(check_linux_library(&csound7), Ok(()));

    let csound6 = scratch.dir("csound6");
    fs::write(csound6.join("libcsound64.so.6.0"), b"").unwrap();
    symlink("libcsound64.so.6.0", csound6.join(LINUX_LIBRARY_NAME)).unwrap();
    assert_eq!(
        check_linux_library(&csound6),
        Err("libcsound64.so points to a Csound 6 library".to_owned())
    );
}

#[test]
fn macos_lib_dir_may_be_the_folder_containing_the_framework() {
    let scratch = Scratch::new("mac-parent");
    let framework = scratch.dir(MACOS_FRAMEWORK_NAME);
    write_framework_version(&framework, "7.0", 7);

    assert_eq!(check_macos_library(&scratch.0), Ok(()));
    assert_eq!(macos_framework_from_lib_dir(&scratch.0), framework);
}

#[test]
fn macos_lib_dir_may_be_the_framework_bundle_itself() {
    let scratch = Scratch::new("mac-bundle");
    let framework = scratch.dir(MACOS_FRAMEWORK_NAME);
    write_headers(&framework.join("Headers"), 7);
    fs::write(framework.join("CsoundLib64"), b"").unwrap();

    assert_eq!(check_macos_library(&framework), Ok(()));
    assert_eq!(macos_framework_from_lib_dir(&framework), framework);
}

#[test]
fn macos_lib_dir_without_a_framework_is_rejected() {
    let scratch = Scratch::new("mac-missing");
    assert_eq!(
        check_macos_library(&scratch.0),
        Err("CsoundLib64.framework not found".to_owned())
    );

    scratch.dir(MACOS_FRAMEWORK_NAME);
    assert_eq!(
        check_macos_library(&scratch.0),
        Err("CsoundLib64.framework has no CsoundLib64 binary".to_owned())
    );
}

#[cfg(unix)]
#[test]
fn macos_framework_is_dated_by_the_version_current_points_to() {
    let scratch = Scratch::new("mac-current");
    let framework = scratch.dir(MACOS_FRAMEWORK_NAME);
    write_framework_version(&framework, "6.0", 6);
    write_framework_version(&framework, "7.0", 7);

    // The linker uses the top-level binary, i.e. Current, even though a
    // Versions/7.0 is present.
    link_framework_current(&framework, "6.0");
    let version_dir = macos_framework_version_dir(&framework).unwrap();
    assert!(version_dir.ends_with("Versions/6.0"), "{version_dir:?}");
    let error = check_macos_library(&scratch.0).unwrap_err();
    assert!(error.ends_with("version.h reports Csound 6"), "{error}");

    link_framework_current(&framework, "7.0");
    assert_eq!(check_macos_library(&scratch.0), Ok(()));
}
