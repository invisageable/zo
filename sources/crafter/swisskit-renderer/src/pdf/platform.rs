//! Platform-specific pdfium library loading.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

use pdfium_render::prelude::*;

use std::path::PathBuf;

/// Returns platform-specific search paths for pdfium library.
/// Paths are tried in order after system library binding fails.
pub fn search_paths() -> Vec<PathBuf> {
  #[cfg(target_os = "macos")]
  return macos::search_paths();

  #[cfg(target_os = "linux")]
  return linux::search_paths();

  #[cfg(target_os = "windows")]
  return windows::search_paths();
}

/// Bind to pdfium library using platform-specific search strategy.
pub fn bind_pdfium() -> Result<Pdfium, PdfiumError> {
  // Try system library first (works on all platforms)
  if let Ok(bindings) = Pdfium::bind_to_system_library() {
    return Ok(Pdfium::new(bindings));
  }

  // Try platform-specific paths
  for path in search_paths() {
    let lib_path = Pdfium::pdfium_platform_library_name_at_path(&path);
    if let Ok(bindings) = Pdfium::bind_to_library(&lib_path) {
      return Ok(Pdfium::new(bindings));
    }
  }

  // Final fallback: current directory
  let path = Pdfium::pdfium_platform_library_name_at_path("./");
  let bindings = Pdfium::bind_to_library(&path)?;

  Ok(Pdfium::new(bindings))
}
