//! Linux-specific pdfium library paths.

use std::path::PathBuf;

/// Returns Linux-specific search paths for pdfium library.
pub fn search_paths() -> Vec<PathBuf> {
  let mut paths = Vec::new();

  // Get executable directory
  if let Ok(exe_path) = std::env::current_exe() {
    if let Some(exe_dir) = exe_path.parent() {
      // Same directory as executable
      paths.push(exe_dir.to_path_buf());

      // AppImage/Flatpak: lib subdirectory
      paths.push(exe_dir.join("lib"));
      paths.push(exe_dir.join("../lib"));
    }
  }

  // Standard Linux library paths
  paths.push(PathBuf::from("/usr/local/lib"));
  paths.push(PathBuf::from("/usr/lib"));
  paths.push(PathBuf::from("/usr/lib64"));

  paths
}
