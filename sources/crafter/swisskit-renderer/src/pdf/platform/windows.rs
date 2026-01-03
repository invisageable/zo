//! Windows-specific pdfium library paths.

use std::path::PathBuf;

/// Returns Windows-specific search paths for pdfium library.
pub fn search_paths() -> Vec<PathBuf> {
  let mut paths = Vec::new();

  // Get executable directory
  if let Ok(exe_path) = std::env::current_exe() {
    if let Some(exe_dir) = exe_path.parent() {
      // Same directory as executable
      paths.push(exe_dir.to_path_buf());
    }
  }

  // Program Files paths
  if let Ok(program_files) = std::env::var("ProgramFiles") {
    paths.push(PathBuf::from(program_files));
  }

  paths
}
