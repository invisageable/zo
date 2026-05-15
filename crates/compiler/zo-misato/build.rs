// libzo_misato calls raylib's 3D primitives (declared as
// `extern "C"` in lib.rs). The cdylib's behaviour depends on
// where raylib lives at link time:
//
//   * If raylib is installed where the linker can find it
//     (homebrew on macOS, libraylib-dev on Debian, an
//     explicit `RAYLIB_LIB_DIR`), we emit `-L<dir> -lraylib`
//     so symbols resolve at link time. Best for prod builds.
//
//   * If raylib isn't installed (CI runners that only build
//     the workspace, dev machines without graphics deps),
//     we emit no raylib directives. The cdylib still links —
//     `-shared` accepts undefined symbols by default — and
//     dyld resolves the references at user-binary load time
//     when both `libzo_misato` and `libraylib` are pulled in.
//
// Detection is path-based: an explicit `RAYLIB_LIB_DIR` env
// var wins, otherwise we probe the canonical system locations
// for the host's library file extension.

use std::path::{Path, PathBuf};

fn main() {
  println!("cargo:rerun-if-env-changed=RAYLIB_LIB_DIR");

  if let Some(dir) = locate_raylib_dir() {
    println!("cargo:rustc-link-search={}", dir.display());
    println!("cargo:rustc-link-lib=raylib");
  }
  // No raylib found: silently skip. The cdylib links with
  // unresolved raylib symbols; the user binary must bring
  // raylib in itself at runtime.
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
