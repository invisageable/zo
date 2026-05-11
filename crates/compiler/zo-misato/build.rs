// Tell cargo to link libzo_misato.dylib against libraylib
// at build time. The Rust runtime calls raylib's 3D
// primitives (BeginMode3D / DrawCubeV / EndMode3D) directly
// — they're declared as `extern "C"` in lib.rs, dyld
// resolves them at user-binary load time.
//
// Path matches `brew install raylib` on Apple Silicon. On
// other systems, set `RAYLIB_LIB_DIR` before building or
// override via `cargo:rustc-link-search` in a downstream
// environment. M1 only targets macOS / homebrew.

fn main() {
  if let Ok(custom) = std::env::var("RAYLIB_LIB_DIR") {
    println!("cargo:rustc-link-search={custom}");
  } else {
    println!("cargo:rustc-link-search=/opt/homebrew/lib");
  }

  println!("cargo:rustc-link-lib=raylib");
}
