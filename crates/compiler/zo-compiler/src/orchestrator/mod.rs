//! Orchestrator API for batch compilation.
//!
//! This module provides a high-level API for build systems like fret to
//! compile batches of source files. It wraps the existing Compiler with a
//! more ergonomic interface for batch operations.

mod batch;

pub use batch::BatchResult;

use crate::Compiler;

use zo_codegen_backend::Target;
use zo_error::{Error, ErrorKind};
use zo_span::Span;

use hashbrown::HashMap;

use std::path::PathBuf;
use std::time::Instant;

/// Orchestrator for batch compilation operations.
///
/// This wraps the existing Compiler and provides a batch-friendly API
/// for build systems. It handles conversion between HashMap-based source
/// maps and the Vec-based API that Compiler expects.
pub struct Orchestrator {
  compiler: Compiler,
}
impl Orchestrator {
  /// Creates a new orchestrator instance.
  pub fn new() -> Self {
    Self {
      compiler: Compiler::new(),
    }
  }

  /// Compiles a batch of source files.
  ///
  /// # Arguments
  /// * `sources` - Map of file paths to source code content
  /// * `target` - Target platform for code generation
  /// * `output_dir` - Optional output directory for generated artifacts
  ///
  /// # Returns
  /// BatchResult containing compilation outcome and statistics
  ///
  /// # Example
  /// ```no_run
  /// use zo_compiler::orchestrator::Orchestrator;
  /// use zo_codegen_backend::Target;
  /// use hashbrown::HashMap;
  /// use std::path::PathBuf;
  ///
  /// let mut orchestrator = Orchestrator::new();
  /// let mut sources = HashMap::new();
  /// sources.insert(
  ///   PathBuf::from("main.zo"),
  ///   "fun main() -> int { return 0; }".to_string()
  /// );
  ///
  /// let result = orchestrator.compile_batch(
  ///   sources,
  ///   Target::host(),
  ///   Some(&PathBuf::from("./build"))
  /// );
  /// assert!(result.is_success());
  /// ```
  pub fn compile_batch(
    &mut self,
    sources: HashMap<PathBuf, String>,
    target: Target,
    output_dir: Option<&PathBuf>,
  ) -> BatchResult {
    let start_time = Instant::now();
    let files_count = sources.len();

    // Convert HashMap to Vec.
    let files_vec: Vec<(PathBuf, String)> = sources.into_iter().collect();

    // For batch compilation with output_dir, we need to compile each file
    // individually with its specific output path in the output directory.
    if let Some(out_dir) = output_dir {
      // Ensure output directory exists
      if std::fs::create_dir_all(out_dir).is_err() {
        let duration = start_time.elapsed();
        let error = Error::new(ErrorKind::InternalCompilerError, Span::ZERO);
        return BatchResult::from_error(error).with_duration(duration);
      }

      for (path, content) in &files_vec {
        // Compute output path: output_dir/filename (without extension).
        let filename = path
          .file_stem()
          .and_then(|s| s.to_str())
          .unwrap_or("output");
        let output_path = out_dir.join(filename);

        // Compile single file.
        let file_ref = vec![(path, content.clone())];
        if let Err(e) =
          self
            .compiler
            .compile(&file_ref, target, &[], &Some(output_path))
        {
          let duration = start_time.elapsed();
          return BatchResult::from_error(e).with_duration(duration);
        }
      }

      let duration = start_time.elapsed();
      return BatchResult::success(files_count).with_duration(duration);
    }

    // No output_dir: use default per-file output (same location as source).
    let files_refs: Vec<(&PathBuf, String)> = files_vec
      .iter()
      .map(|(path, content)| (path, content.clone()))
      .collect();

    let result = match self.compiler.compile(&files_refs, target, &[], &None) {
      Ok(()) => BatchResult::success(files_count),
      Err(e) => BatchResult::from_error(e),
    };

    let duration = start_time.elapsed();
    result.with_duration(duration)
  }
}
impl Default for Orchestrator {
  fn default() -> Self {
    Self::new()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_compile_batch_empty() {
    let mut orchestrator = Orchestrator::new();
    let result =
      orchestrator.compile_batch(HashMap::new(), Target::host(), None);

    assert!(result.is_success());
    assert_eq!(result.files_compiled(), 0);
  }
}
