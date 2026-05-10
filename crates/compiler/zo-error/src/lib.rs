mod error;

pub use error::{Error, ErrorKind, Severity, severity};

pub type Result<T> = anyhow::Result<T, Vec<Error>>;

#[cfg(test)]
mod tests {
  use super::*;

  /// Locks the `Error` packing invariant — any new field would
  /// force alignment padding to 24 bytes, growing the
  /// collector's `[Error; 128]` buffer by 50% to 3 KiB per
  /// thread. Severity must stay derived (see `severity()`),
  /// never stored.
  #[test]
  fn error_struct_size_is_16_bytes() {
    assert_eq!(std::mem::size_of::<Error>(), 16);
  }

  #[test]
  fn severity_classifies_warnings_and_errors() {
    assert_eq!(severity(ErrorKind::UnusedVariable), Severity::Warning);
    assert_eq!(severity(ErrorKind::UnusedFunction), Severity::Warning);
    assert_eq!(severity(ErrorKind::UnreachableCode), Severity::Warning);
    assert_eq!(severity(ErrorKind::TypeMismatch), Severity::Error);
    assert_eq!(severity(ErrorKind::NonExhaustiveMatch), Severity::Error);
  }
}
