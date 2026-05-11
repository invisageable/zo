// libzo_misato.dylib calls raylib's 3D primitives (declared
// as `extern "C"` in lib.rs); link against libraylib so the
// references resolve at the user binary's load time.
//
// Default search path matches `brew install raylib` on
// Apple Silicon. Override with `RAYLIB_LIB_DIR=...` on
// other platforms.

fn main() {
  if let Ok(custom) = std::env::var("RAYLIB_LIB_DIR") {
    println!("cargo:rustc-link-search={custom}");
  } else {
    println!("cargo:rustc-link-search=/opt/homebrew/lib");
  }

  println!("cargo:rustc-link-lib=raylib");
}
