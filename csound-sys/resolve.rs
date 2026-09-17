// Shared Csound 7 discovery helpers for `build.rs`.
//
// When both `CSOUND_INCLUDE_DIR` and `CSOUND_LIB_DIR` are set they name an
// explicit install and must win over pkg-config, `/Applications/Csound`,
// Program Files, and other well-known locations. Setting only one of them, or
// naming an incomplete or pre-7 install, must fail rather than silently fall
// through to a leftover system install.
//
// The `check_*` functions return the reason a directory is unusable so build
// failures can say what is wrong, not just that something is.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

pub const LINUX_LIBRARY_NAME: &str = "libcsound64.so";
pub const WINDOWS_LIBRARY_NAME: &str = "csound64.lib";
pub const MACOS_FRAMEWORK_NAME: &str = "CsoundLib64.framework";

/// Treats a variable set to the empty string like an unset one.
pub fn env_path(value: Option<OsString>) -> Option<PathBuf> {
    value.filter(|value| !value.is_empty()).map(PathBuf::from)
}

pub fn csound_major_version_from_contents(contents: &str) -> Option<u32> {
    let definition = contents
        .lines()
        .find(|line| line.trim_start().starts_with("#define CS_VERSION"))?;

    definition
        .split(|character: char| !character.is_ascii_digit())
        .find(|part| !part.is_empty())?
        .parse()
        .ok()
}

pub fn csound_major_version(include_dir: &Path) -> Option<u32> {
    let contents = std::fs::read_to_string(include_dir.join("version.h")).ok()?;
    csound_major_version_from_contents(&contents)
}

/// Checks that `include_dir` holds the Csound 7 public headers.
pub fn check_include_dir(include_dir: &Path) -> Result<(), String> {
    if !include_dir.join("csound.h").is_file() {
        return Err("csound.h not found".to_owned());
    }
    match csound_major_version(include_dir) {
        Some(major) if major >= 7 => Ok(()),
        Some(major) => Err(format!("version.h reports Csound {major}")),
        None => Err("version.h not found, or it does not define CS_VERSION".to_owned()),
    }
}

/// Checks that `library_dir` holds a Csound 7 `libcsound64.so`.
///
/// Csound installs `libcsound64.so` as a link to the library named after its
/// API version (`libcsound64.so.7.0`), so that name dates the library. A
/// library without a version suffix cannot be dated and is accepted.
pub fn check_linux_library(library_dir: &Path) -> Result<(), String> {
    let library = library_dir.join(LINUX_LIBRARY_NAME);
    if !library.is_file() {
        return Err(format!("{LINUX_LIBRARY_NAME} not found"));
    }
    if let Some(major) = linux_library_major_version(&library)
        && major < 7
    {
        return Err(format!(
            "{LINUX_LIBRARY_NAME} points to a Csound {major} library"
        ));
    }
    Ok(())
}

fn linux_library_major_version(library: &Path) -> Option<u32> {
    let resolved = library.canonicalize().ok()?;
    resolved
        .file_name()?
        .to_str()?
        .strip_prefix(LINUX_LIBRARY_NAME)?
        .strip_prefix('.')?
        .split('.')
        .next()?
        .parse()
        .ok()
}

/// Checks that `library_dir` holds `csound64.lib`.
///
/// An import library does not record the Csound version, so only the headers
/// are version-checked on Windows.
pub fn check_windows_library(library_dir: &Path) -> Result<(), String> {
    if library_dir.join(WINDOWS_LIBRARY_NAME).is_file() {
        Ok(())
    } else {
        Err(format!("{WINDOWS_LIBRARY_NAME} not found"))
    }
}

/// `CSOUND_LIB_DIR` may be either the directory containing the framework or
/// the framework bundle itself.
pub fn macos_framework_from_lib_dir(library_dir: &Path) -> PathBuf {
    if library_dir
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == MACOS_FRAMEWORK_NAME)
    {
        library_dir.to_path_buf()
    } else {
        library_dir.join(MACOS_FRAMEWORK_NAME)
    }
}

/// Returns the directory holding the framework binary, with symlinks such as
/// `Versions/Current` resolved.
///
/// Its `Headers` describe the binary the linker will use. A framework can hold
/// a `Versions/7.0` next to a `Current` that still points at Csound 6, so the
/// presence of `Versions/7.0` alone proves nothing.
pub fn macos_framework_version_dir(framework: &Path) -> Option<PathBuf> {
    let binary = [
        framework.join("CsoundLib64"),
        framework.join("Versions/7.0/CsoundLib64"),
    ]
    .into_iter()
    .find(|binary| binary.is_file())?;
    Some(binary.canonicalize().ok()?.parent()?.to_path_buf())
}

/// Checks that `library_dir` is, or contains, a Csound 7 framework.
pub fn check_macos_library(library_dir: &Path) -> Result<(), String> {
    let framework = macos_framework_from_lib_dir(library_dir);
    if !framework.is_dir() {
        return Err(format!("{MACOS_FRAMEWORK_NAME} not found"));
    }
    let version_dir = macos_framework_version_dir(&framework)
        .ok_or_else(|| format!("{MACOS_FRAMEWORK_NAME} has no CsoundLib64 binary"))?;
    let headers = version_dir.join("Headers");
    check_include_dir(&headers)
        .map_err(|reason| format!("in the linked {}, {reason}", headers.display()))
}

/// Resolves the explicit `CSOUND_INCLUDE_DIR` + `CSOUND_LIB_DIR` pair.
///
/// Returns `Ok(None)` when neither is set, so well-known locations can be
/// searched. Setting only one, or naming directories that do not hold Csound
/// 7, is an error: falling through would silently use whichever other
/// installation happens to exist.
pub fn explicit_csound7_dirs(
    include_dir: Option<PathBuf>,
    library_dir: Option<PathBuf>,
    check_library: impl Fn(&Path) -> Result<(), String>,
) -> Result<Option<(PathBuf, PathBuf)>, String> {
    let (include_dir, library_dir) = match (include_dir, library_dir) {
        (None, None) => return Ok(None),
        (Some(include_dir), Some(library_dir)) => (include_dir, library_dir),
        (Some(_), None) => return Err(only_one_set("CSOUND_INCLUDE_DIR", "CSOUND_LIB_DIR")),
        (None, Some(_)) => return Err(only_one_set("CSOUND_LIB_DIR", "CSOUND_INCLUDE_DIR")),
    };

    check_include_dir(&include_dir).map_err(|reason| {
        format!(
            "CSOUND_INCLUDE_DIR ({}) is not a Csound 7 header directory: {reason}",
            include_dir.display()
        )
    })?;
    check_library(&library_dir).map_err(|reason| {
        format!(
            "CSOUND_LIB_DIR ({}) is not a Csound 7 library directory: {reason}",
            library_dir.display()
        )
    })?;

    Ok(Some((include_dir, library_dir)))
}

fn only_one_set(set: &str, unset: &str) -> String {
    format!(
        "{set} is set but {unset} is not. Set both to use a custom Csound 7 installation, or \
         unset {set} to search the standard locations."
    )
}
