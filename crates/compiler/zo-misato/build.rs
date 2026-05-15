// libzo_misato calls raylib's 3D primitives (declared as
// `extern "C"` in lib.rs); link-time resolution against
// `libraylib` is mandatory — the macOS Mach-O linker rejects
// unresolved symbols in `-dynamiclib` by default, and Linux
// builds without `-lraylib` produce a cdylib that can't load.
//
// Lookup order:
//
//   1. `RAYLIB_LIB_DIR` env var (CI workflows, packagers).
//   2. Canonical per-host system paths (homebrew on macOS,
//      `libraylib-dev` on Debian).
//
// If neither finds raylib, fail the build with a clear
// install hint rather than silently emit a broken artifact.

use std::path::{Path, PathBuf};

fn main() {
  println!("cargo:rerun-if-env-changed=RAYLIB_LIB_DIR");

  let Some(dir) = locate_raylib_dir() else {
    panic!(
      "raylib not found.\n\
       Set RAYLIB_LIB_DIR=<path-to-dir-containing-libraylib.{{dylib,so}}>,\n\
       or install raylib via your package manager:\n  \
         macOS:  brew install raylib\n  \
         Debian: sudo apt-get install libraylib-dev\n  \
         Other:  https://github.com/raysan5/raylib/releases\n"
    );
  };

  println!("cargo:rustc-link-search={}", dir.display());
  println!("cargo:rustc-link-lib=raylib");
}

/// Returns the first directory that contains a usable
/// `libraylib.{dylib,so,a}` for the host. `RAYLIB_LIB_DIR`
/// always wins — caller-provided paths are trusted without
/// re-checking the file exists, matching the rustc convention
/// for build-script overrides.
fn locate_raylib_dir() -> Option<PathBuf> {
  if let Ok(custom) = std::env::var("RAYLIB_LIB_DIR") {
    return Some(PathBuf::from(custom));
  }

  // Per-host candidates. macOS uses `.dylib`, Linux `.so`.
  // Apple Silicon homebrew lives under `/opt/homebrew`; Intel
  // homebrew under `/usr/local`. Debian's libraylib-dev lands
  // under `/usr/lib/<triple>`.
  #[cfg(target_os = "macos")]
  let candidates: &[(&str, &str)] = &[
    ("/opt/homebrew/lib", "libraylib.dylib"),
    ("/usr/local/lib", "libraylib.dylib"),
  ];

  #[cfg(target_os = "linux")]
  let candidates: &[(&str, &str)] = &[
    ("/usr/lib/x86_64-linux-gnu", "libraylib.so"),
    ("/usr/lib/aarch64-linux-gnu", "libraylib.so"),
    ("/usr/local/lib", "libraylib.so"),
    ("/usr/lib", "libraylib.so"),
  ];

  #[cfg(not(any(target_os = "macos", target_os = "linux")))]
  let candidates: &[(&str, &str)] = &[];

  for (dir, file) in candidates {
    let path = Path::new(dir).join(file);
    if path.exists() {
      return Some(PathBuf::from(dir));
    }
  }

  None
}
