//! Link the system `libraylib` so a texture shares the one
//! GL context the window already created — not a second,
//! bundled copy.
fn main() {
  if cfg!(target_os = "macos") {
    println!("cargo:rustc-link-search=native=/opt/homebrew/lib");
  } else {
    println!("cargo:rustc-link-search=native=/usr/lib/x86_64-linux-gnu");
  }

  println!("cargo:rustc-link-lib=dylib=raylib");
}
