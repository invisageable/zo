//! Link the system `libraylib` so a texture shares the one
//! GL context the window already created — not a second,
//! bundled copy.
//!
//! Lookup order matches `zo-misato/build.rs` — both cdylibs
//! must resolve raylib at link time, and CI / packagers point
//! at one install via `RAYLIB_LIB_DIR`:
//!
//!   1. `RAYLIB_LIB_DIR` env var (CI workflows, packagers).
//!   2. Canonical per-host paths (homebrew on macOS,
//!      `libraylib-dev` on Debian, raysan5 prebuilts).
//!
//! Fail with an install hint rather than emit a cdylib that
//! can't load.

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

  // Bake an `LC_RPATH` / `DT_RUNPATH` at the resolved raylib
  // directory so the cdylib finds `libraylib` at load time on
  // distributions whose `install_name` / SONAME is expressed
  // as `@rpath/libraylib.<ver>.dylib` — e.g. the raysan5
  // prebuilt tarballs CI and `release.yml` use.
  println!("cargo:rustc-link-arg=-Wl,-rpath,{}", dir.display());
}

/// First directory containing a usable `libraylib.{dylib,so}`
/// for the host. `RAYLIB_LIB_DIR` always wins — the build-
/// script override convention; otherwise probe per-host paths.
fn locate_raylib_dir() -> Option<PathBuf> {
  if let Ok(custom) = std::env::var("RAYLIB_LIB_DIR") {
    return Some(PathBuf::from(custom));
  }

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
    if Path::new(dir).join(file).exists() {
      return Some(PathBuf::from(dir));
    }
  }

  None
}
