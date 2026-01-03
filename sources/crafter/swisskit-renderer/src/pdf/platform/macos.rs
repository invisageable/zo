//! macOS-specific pdfium library paths.

use std::path::PathBuf;

/// Returns macOS-specific search paths for pdfium library.
pub fn search_paths() -> Vec<PathBuf> {
  let mut paths = Vec::new();

  // Get executable directory
  if let Ok(exe_path) = std::env::current_exe()
    && let Some(exe_dir) = exe_path.parent()
  {
    paths.push(exe_dir.to_path_buf()); // Same directory as executable
    paths.push(exe_dir.join("../Frameworks")); // macOS app bundle: Contents/Frameworks/
    paths.push(exe_dir.join("../Resources")); // macOS app bundle: Contents/Resources/
  }

  // Homebrew paths
  paths.push(PathBuf::from("/opt/homebrew/lib"));
  paths.push(PathBuf::from("/usr/local/lib"));

  paths
}
