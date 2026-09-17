// Shared Csound 7 discovery helpers for `build.rs`.
//
// When both `CSOUND_INCLUDE_DIR` and `CSOUND_LIB_DIR` are set they name an
// explicit install and must win over pkg-config, `/Applications/Csound`,
// Program Files, and other well-known locations. A pair that is set but
// incomplete (Csound 6, missing library) must fail rather than silently
// falling through to a leftover system install.

use std::path::{Path, PathBuf};

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

pub fn is_csound7_include_dir(include_dir: &Path) -> bool {
    include_dir.join("csound.h").is_file()
        && csound_major_version(include_dir).is_some_and(|major| major >= 7)
}

pub const MACOS_FRAMEWORK_NAME: &str = "CsoundLib64.framework";

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

pub fn macos_framework_binary_exists(framework: &Path) -> bool {
    framework.join("CsoundLib64").is_file() || framework.join("Versions/7.0/CsoundLib64").is_file()
}

pub fn macos_library_ok(library_dir: &Path) -> bool {
    macos_framework_binary_exists(&macos_framework_from_lib_dir(library_dir))
}

pub fn linux_library_ok(library_dir: &Path) -> bool {
    let dylib_name = format!(
        "{}csound64{}",
        std::env::consts::DLL_PREFIX,
        std::env::consts::DLL_SUFFIX
    );
    library_dir.join(dylib_name).is_file()
}

pub fn windows_library_ok(library_dir: &Path) -> bool {
    library_dir.join("csound64.lib").is_file()
}

/// Both directories are set and name a complete Csound 7 header/library pair.
pub fn explicit_csound7_dirs(
    include_dir: Option<&Path>,
    library_dir: Option<&Path>,
    library_ok: impl Fn(&Path) -> bool,
) -> Option<(PathBuf, PathBuf)> {
    let include_dir = include_dir?;
    let library_dir = library_dir?;
    if is_csound7_include_dir(include_dir) && library_ok(library_dir) {
        Some((include_dir.to_path_buf(), library_dir.to_path_buf()))
    } else {
        None
    }
}

/// Use an explicit `CSOUND_*` pair when both are set.
///
/// If only one is set, returns `None` so well-known locations can still be
/// searched. If both are set but the pair is not a complete Csound 7 install,
/// panics instead of falling through to a leftover system copy.
pub fn try_explicit_csound7_env(
    include_dir: Option<&Path>,
    library_dir: Option<&Path>,
    library_ok: impl Fn(&Path) -> bool,
) -> Option<(PathBuf, PathBuf)> {
    let (Some(include_dir), Some(library_dir)) = (include_dir, library_dir) else {
        return None;
    };
    if let Some(pair) = explicit_csound7_dirs(Some(include_dir), Some(library_dir), library_ok) {
        return Some(pair);
    }
    if !is_csound7_include_dir(include_dir) {
        panic!(
            "CSOUND_INCLUDE_DIR ({}) and CSOUND_LIB_DIR are both set, but the include directory is not a complete Csound 7 header set (need csound.h and version.h with CS_VERSION >= 7).",
            include_dir.display()
        );
    }
    panic!(
        "CSOUND_INCLUDE_DIR and CSOUND_LIB_DIR ({}) are both set, but the library directory does not contain the Csound 7 library for this platform.",
        library_dir.display()
    );
}
