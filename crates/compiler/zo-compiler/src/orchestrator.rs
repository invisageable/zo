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

use std::fs;
use std::path::{Path, PathBuf};
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
  /// * `sources`        — Map of file paths to source content.
  /// * `target`         — Target platform for code generation.
  /// * `out_dir`        — Optional `--out-dir`: directory every
  ///   emitted file lands in.
  /// * `entry_binary`   — Optional `(entry_source, binary_path)`:
  ///   for the source file at `entry_source`, write the final
  ///   binary to `binary_path`. Other sources in the batch use
  ///   default placement (`<out_dir>/<stem>` or next to source).
  ///   This is the cargo / build-system shape: "this is the
  ///   entry point and here's where its binary goes," other
  ///   files are just along for the ride.
  ///
  /// # Returns
  /// BatchResult containing compilation outcome and statistics
  pub fn compile_batch(
    &mut self,
    sources: HashMap<PathBuf, String>,
    target: Target,
    out_dir: Option<&Path>,
    entry_binary: Option<(&Path, &Path)>,
  ) -> BatchResult {
    let start_time = Instant::now();
    let files_count = sources.len();
    let files_vec = sources.into_iter().collect::<Vec<_>>();

    if let Some(dir) = out_dir
      && fs::create_dir_all(dir).is_err()
    {
      let duration = start_time.elapsed();
      let error = Error::new(ErrorKind::InternalCompilerError, Span::ZERO);
      return BatchResult::from_error(error).with_duration(duration);
    }

    // Per-file compilation lets us route the entry-point
    // binary to `binary_path` while every other source falls
    // through to default `<out_dir>/<stem>` placement —
    // matches the `(entry_source, binary_path)` contract
    // above. Single batch invocation of `Compiler::compile`
    // would force one `output_path` across every source.
    for (path, content) in &files_vec {
      let explicit_binary = match entry_binary {
        Some((entry_src, binary_path)) if entry_src == path.as_path() => {
          Some(binary_path.to_path_buf())
        }
        _ => None,
      };

      let file_ref = vec![(path, content.clone())];

      if let Err(e) =
        self
          .compiler
          .compile(&file_ref, target, &[], &explicit_binary, out_dir)
      {
        let duration = start_time.elapsed();
        return BatchResult::from_error(e).with_duration(duration);
      }
    }

    let duration = start_time.elapsed();
    BatchResult::success(files_count).with_duration(duration)
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
      orchestrator.compile_batch(HashMap::new(), Target::host(), None, None);

    assert!(result.is_success());
    assert_eq!(result.files_compiled(), 0);
  }
}
