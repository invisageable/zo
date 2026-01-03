//! Source file collection stage for fret.
//!
//! This stage discovers all .zo source files in the project,
//! using a fast directory traversal with minimal allocations.

use crate::types::{BuildContext, Stage, StageError};

use std::fs;
use std::path::{Path, PathBuf};

/// Stage that collects all .zo source files from the source directory.
pub struct CollectSources;

impl Stage for CollectSources {
  fn execute(&self, ctx: &mut BuildContext) -> Result<(), StageError> {
    // Determine source directory
    let source_dir = if ctx.config.source_dir.is_absolute() {
      ctx.config.source_dir.clone()
    } else {
      ctx.project_root.join(&ctx.config.source_dir)
    };

    if !source_dir.exists() {
      return Err(StageError::SourceCollection(format!(
        "Source directory not found: {}",
        source_dir.display()
      )));
    }

    // Pre-allocate with reasonable capacity to minimize reallocations
    ctx.source_files.clear();
    ctx.source_files.reserve(128);

    // Collect all .zo files recursively
    collect_zo_files(&source_dir, &mut ctx.source_files)?;

    // Ensure entry point exists
    let entry_point = if ctx.config.entry_point.is_absolute() {
      ctx.config.entry_point.clone()
    } else {
      ctx.project_root.join(&ctx.config.entry_point)
    };

    if !entry_point.exists() {
      return Err(StageError::SourceCollection(format!(
        "Entry point not found: {}",
        entry_point.display()
      )));
    }

    // Add entry point if not already in the list
    if !ctx.source_files.contains(&entry_point) {
      ctx.source_files.push(entry_point);
    }

    // Sort files for deterministic builds
    ctx.source_files.sort_unstable();

    Ok(())
  }

  fn name(&self) -> &'static str {
    "CollectSources"
  }
}

/// Recursively collect all .zo files in a directory.
/// Uses a manual stack to avoid recursion overhead.
fn collect_zo_files(
  root: &Path,
  files: &mut Vec<PathBuf>,
) -> Result<(), StageError> {
  // Manual stack for directory traversal - avoids recursion
  let mut dir_stack = vec![root.to_path_buf()];

  while let Some(dir) = dir_stack.pop() {
    let entries = fs::read_dir(&dir).map_err(|error| {
      StageError::SourceCollection(format!(
        "Failed to read directory {}: {error}",
        dir.display(),
      ))
    })?;

    for entry in entries {
      let entry = entry.map_err(|error| {
        StageError::SourceCollection(format!(
          "Failed to read entry in {}: {error}",
          dir.display(),
        ))
      })?;

      let path = entry.path();
      let file_type = entry.file_type().map_err(|error| {
        StageError::SourceCollection(format!(
          "Failed to get file type for {}: {error}",
          path.display(),
        ))
      })?;

      if file_type.is_dir() {
        // Skip hidden directories and common build directories
        if let Some(name) = path.file_name() {
          let name_str = name.to_string_lossy();
          if !name_str.starts_with('.')
            && name_str != "build"
            && name_str != "target"
          {
            dir_stack.push(path);
          }
        }
      } else if file_type.is_file() {
        // Check if it's a .zo file
        if path.extension().map_or(false, |ext| ext == "zo") {
          files.push(path);
        }
      }
    }
  }

  Ok(())
}
