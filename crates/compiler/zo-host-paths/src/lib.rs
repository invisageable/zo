//! Resolve directories relative to the running zo
//! binary's executable path. Three consumers share this
//! shape — std-lib search paths (`zo-compiler`,
//! `zo-driver`) and vendored-prebuilt resolution
//! (`zo-executor` for the F7 `#link` fallback).
//!
//! `current_exe()` is a syscall on macOS
//! (`_NSGetExecutablePath` + `realpath`); callers that
//! call into here in a hot path should cache the result.

use std::path::PathBuf;

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
