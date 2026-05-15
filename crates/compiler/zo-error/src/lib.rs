mod error;
pub mod id_registry;

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

  /// Locks the frozen-contract on a handful of representative
  /// variants from every phase range. Reordering `ErrorKind`
  /// must not move any of these ids or codes; renaming a
  /// variant must add a new id rather than mutate an existing
  /// one. Snapshot kept short on purpose — the full table
  /// is the `id_registry::entry` match itself.
  #[test]
  fn id_registry_is_frozen() {
    let pairs: &[(ErrorKind, &str, u16)] = &[
      // Tokenizer.
      (ErrorKind::UnexpectedCharacter, "unexpected-character", 1),
      (ErrorKind::UnterminatedString, "unterminated-string", 2),
      // Parser.
      (ErrorKind::UnexpectedToken, "unexpected-token", 100),
      (ErrorKind::ExpectedSemicolon, "expected-semicolon", 131),
      // Analyzer.
      (ErrorKind::TypeMismatch, "type-mismatch", 304),
      (ErrorKind::UndefinedVariable, "undefined-variable", 301),
      (ErrorKind::ImmutableVariable, "immutable-variable", 309),
      (ErrorKind::NonExhaustiveMatch, "non-exhaustive-match", 333),
      // Constants & arithmetic.
      (ErrorKind::DivisionByZero, "division-by-zero", 500),
      // Codegen / Linker / Internal.
      (
        ErrorKind::InternalCompilerError,
        "internal-compiler-error",
        605,
      ),
      // Modules / FFI / Concurrency.
      (
        ErrorKind::LinkResolutionFailed,
        "link-resolution-failed",
        704,
      ),
      (ErrorKind::SpawnOutsideNursery, "spawn-outside-nursery", 705),
      // Entry point & misc.
      (ErrorKind::MissingMainFunction, "missing-main-function", 800),
    ];

    for (kind, expected_id, expected_code) in pairs {
      assert_eq!(
        kind.id(),
        *expected_id,
        "id changed for {kind:?} — frozen contract violated"
      );
      assert_eq!(
        kind.code(),
        *expected_code,
        "code changed for {kind:?} — frozen contract violated"
      );
    }
  }

  /// Numeric codes must fit the documented phase ranges
  /// (E0001..E0899). New variants picking codes outside
  /// these ranges break the human-readable phase-grouping
  /// convention.
  #[test]
  fn id_registry_codes_within_documented_ranges() {
    // Sentinel variants at the boundaries of each documented range.
    assert!(ErrorKind::UnexpectedCharacter.code() < 100);
    assert!((100..300).contains(&ErrorKind::UnexpectedToken.code()));
    assert!((300..500).contains(&ErrorKind::TypeMismatch.code()));
    assert!((500..600).contains(&ErrorKind::DivisionByZero.code()));
    assert!((600..700).contains(&ErrorKind::InternalCompilerError.code()));
    assert!((700..800).contains(&ErrorKind::LinkResolutionFailed.code()));
    assert!((800..900).contains(&ErrorKind::MissingMainFunction.code()));
  }
}
