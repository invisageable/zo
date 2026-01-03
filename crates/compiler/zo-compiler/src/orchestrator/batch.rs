//! Batch compilation result types.

use zo_error::Error;

use std::time::Duration;

/// Result of a batch compilation operation.
///
/// Contains information about the compilation outcome, including success
/// status, errors, and statistics.
#[derive(Debug)]
pub struct BatchResult {
  /// Whether compilation succeeded.
  pub success: bool,
  /// Compilation errors (empty if success).
  pub errors: Vec<Error>,
  /// Number of files compiled.
  pub files_compiled: usize,
  /// Total compilation time.
  pub duration: Duration,
}
impl BatchResult {
  /// Creates a successful batch result.
  ///
  /// # Arguments
  /// * `files_compiled` - Number of files successfully compiled
  pub fn success(files_compiled: usize) -> Self {
    Self {
      success: true,
      errors: Vec::new(),
      files_compiled,
      duration: Duration::ZERO,
    }
  }

  /// Creates a batch result from a compilation error.
  ///
  /// # Arguments
  /// * `error` - The compilation error that occurred
  pub fn from_error(error: Error) -> Self {
    Self {
      success: false,
      errors: vec![error],
      files_compiled: 0,
      duration: Duration::ZERO,
    }
  }

  /// Sets the compilation duration.
  pub fn with_duration(mut self, duration: Duration) -> Self {
    self.duration = duration;
    self
  }

  /// Checks if compilation succeeded.
  pub fn is_success(&self) -> bool {
    self.success
  }

  /// Gets the compilation errors.
  pub fn errors(&self) -> &[Error] {
    &self.errors
  }

  /// Gets the number of files compiled.
  pub fn files_compiled(&self) -> usize {
    self.files_compiled
  }

  /// Gets the compilation duration.
  pub fn duration(&self) -> Duration {
    self.duration
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use zo_error::ErrorKind;
  use zo_span::Span;

  #[test]
  fn test_success_result() {
    let result = BatchResult::success(5);

    assert!(result.is_success());
    assert_eq!(result.files_compiled(), 5);
    assert!(result.errors().is_empty());
  }

  #[test]
  fn test_error_result() {
    let error = Error::new(ErrorKind::InternalCompilerError, Span::ZERO);
    let result = BatchResult::from_error(error);

    assert!(!result.is_success());
    assert_eq!(result.files_compiled(), 0);
    assert_eq!(result.errors().len(), 1);
  }

  #[test]
  fn test_with_duration() {
    let duration = Duration::from_secs(1);
    let result = BatchResult::success(3).with_duration(duration);

    assert_eq!(result.duration(), duration);
  }
}
