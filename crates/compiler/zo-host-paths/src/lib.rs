//! Resolve directories relative to the running zo
//! binary's executable path. Three consumers share this
//! shape — core-lib search paths (`zo-compiler`,
//! `zo-driver`) and vendored-prebuilt resolution
//! (`zo-executor` for the F7 `#link` fallback).
//!
//! `current_exe()` is a syscall on macOS
//! (`_NSGetExecutablePath` + `realpath`); callers that
//! call into here in a hot path should cache the result.

use std::path::PathBuf;

/// Top-level packages shipped with the zo distribution.
/// Single source of truth for distribution-pack discovery
/// (`existing_lib_dirs`) AND for the user-project
/// load-gate exemption (loads through these roots bypass
/// the project lib.zo's pack declarations).
pub const SYSTEM_PACK_ROOTS: &[&str] = &["core", "provider"];

/// Candidate directories for a `<subdir>` shipped with
/// the zo distribution, in priority order:
///
/// 1. `<exe-dir>/../lib/<subdir>` — installed layout
///    (`tasks/zo-install.sh` writes here).
/// 2. `<exe-dir>/../../crates/compiler-lib/<subdir>` —
///    dev layout (works for `target/debug/zo` and
///    `target/debug/fret`, both at the same depth).
///
/// Returns an empty `Vec` when `current_exe()` fails
/// (sandboxed test runners, exotic platforms). Caller
/// filters by `.is_dir()` / `.exists()`.
pub fn exe_relative_lib_dirs(subdir: &str) -> Vec<PathBuf> {
  let mut out = Vec::new();
  let Ok(exe) = std::env::current_exe() else {
    return out;
  };
  let Some(parent) = exe.parent() else {
    return out;
  };

  out.push(parent.join("..").join("lib").join(subdir));
  out.push(
    parent
      .join("..")
      .join("..")
      .join("crates")
      .join("compiler-lib")
      .join(subdir),
  );

  out
}

/// First [`exe_relative_lib_dirs`] candidate that exists
/// on disk. `None` when `current_exe()` fails or neither
/// installed nor dev layout has the directory.
pub fn first_existing_lib_dir(subdir: &str) -> Option<PathBuf> {
  exe_relative_lib_dirs(subdir)
    .into_iter()
    .find(|p| p.is_dir())
}

/// Existing distribution-root directories for every entry
/// in `subdirs`, preserving caller order. One `current_exe()`
/// syscall regardless of `subdirs.len()`.
pub fn existing_lib_dirs(subdirs: &[&str]) -> Vec<PathBuf> {
  let Ok(exe) = std::env::current_exe() else {
    return Vec::new();
  };
  let Some(parent) = exe.parent() else {
    return Vec::new();
  };

  let mut out = Vec::new();

  for subdir in subdirs {
    let installed = parent.join("..").join("lib").join(subdir);

    if installed.is_dir() {
      out.push(installed);
      continue;
    }

    let dev = parent
      .join("..")
      .join("..")
      .join("crates")
      .join("compiler-lib")
      .join(subdir);

    if dev.is_dir() {
      out.push(dev);
    }
  }

  out
}

/// The cross-compiled runtime dylib filename. Apple-only for now
/// (iOS bundling), so always the Mach-O `.dylib` extension.
const RUNTIME_DYLIB: &str = "libzo_runtime.dylib";

/// Candidate paths for the cross-compiled runtime dylib of `triple`,
/// in priority order:
///
/// 1. `<exe-dir>/../lib/runtime/<triple>/libzo_runtime.dylib` —
///    installed layout (`tasks/zo-install.sh` stages here).
/// 2. `<exe-dir>/../<triple>/<profile>/libzo_runtime.dylib` — dev
///    layout, where cargo cross-builds the cdylib as a sibling of
///    the compiler's own `target/<profile>/zo`.
///
/// The runtime dylib's dev location is `target/<triple>/<profile>/`,
/// not `crates/compiler-lib/`, so this can't reuse
/// [`exe_relative_lib_dirs`]. Returns an empty `Vec` when
/// `current_exe()` fails.
pub fn runtime_dylib_candidates(triple: &str) -> Vec<PathBuf> {
  let mut out = Vec::new();
  let Ok(exe) = std::env::current_exe() else {
    return out;
  };
  let Some(dir) = exe.parent() else {
    return out;
  };

  out.push(
    dir
      .join("..")
      .join("lib")
      .join("runtime")
      .join(triple)
      .join(RUNTIME_DYLIB),
  );

  if let (Some(target_root), Some(profile)) = (dir.parent(), dir.file_name()) {
    out.push(target_root.join(triple).join(profile).join(RUNTIME_DYLIB));
  }

  out
}

/// First [`runtime_dylib_candidates`] entry that exists on disk —
/// the installed sysroot copy if present, else the in-repo build.
/// `None` when `current_exe()` fails or neither layout has it.
pub fn first_existing_runtime_dylib(triple: &str) -> Option<PathBuf> {
  runtime_dylib_candidates(triple)
    .into_iter()
    .find(|p| p.is_file())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn runtime_dylib_candidates_shape() {
    // `current_exe()` resolves under the test runner, so both the
    // installed and dev candidates are present.
    let candidates = runtime_dylib_candidates("aarch64-apple-ios-sim");

    assert_eq!(candidates.len(), 2);

    let installed = candidates[0].to_string_lossy();

    assert!(
      installed
        .ends_with("lib/runtime/aarch64-apple-ios-sim/libzo_runtime.dylib",),
      "installed candidate shape: {installed}",
    );

    let dev = candidates[1].to_string_lossy();

    assert!(
      dev.contains("aarch64-apple-ios-sim")
        && dev.ends_with("libzo_runtime.dylib"),
      "dev candidate shape: {dev}",
    );
  }
}
