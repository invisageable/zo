//! On-disk bundling for distributable app targets.
//!
//! Turns a linked, ad-hoc-signed Mach-O binary plus the platform
//! runtime dylib into a runnable app container — an `.app` directory
//! for iOS (mobile) and macOS (desktop webview). The compiler calls the
//! iOS path after linking for a mobile `--target`; the driver calls the
//! macOS path when packaging a `--target webview` desktop app.

pub mod ios;
pub mod macos;

use std::fs;
use std::io;
use std::path::Path;

/// The reverse-DNS bundle identifier for a program named `name`, e.g.
/// `house.compilords.counter`. Shared by every app container.
pub fn bundle_id(name: &str) -> String {
  format!("house.compilords.{name}")
}

/// Mark a copied executable `rwxr-xr-x` — `fs::copy` drops the bit on
/// some filesystems and the loader refuses a non-executable. Shared by
/// the iOS and macOS bundlers.
pub(crate) fn set_executable(path: &Path) -> io::Result<()> {
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;

    let mut perms = fs::metadata(path)?.permissions();

    perms.set_mode(0o755);
    fs::set_permissions(path, perms)?;
  }

  Ok(())
}
