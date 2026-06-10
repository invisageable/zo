//! ```sh
//! cargo test -p zo-checker --lib tests::naming
//! ```

use super::common::drained_rename;

use crate::Checker;

use zo_error::{ErrorKind, Severity, severity};
use zo_reporter::clear_errors;
use zo_span::Span;

#[test]
fn type_name_must_be_pascal_case() {
  clear_errors();

  let checker = Checker::new();

  checker.check_type_name("my_point", Span::ZERO, 0);

  let (kind, rename) = drained_rename().unwrap();

  assert_eq!(kind, ErrorKind::NonPascalCaseName);
  assert_eq!(rename, "MyPoint");
}

#[test]
fn pascal_case_type_name_is_clean() {
  clear_errors();

  let checker = Checker::new();

  checker.check_type_name("MyPoint", Span::ZERO, 0);
  checker.check_type_name("Point", Span::ZERO, 0);

  assert!(drained_rename().is_none());
}

#[test]
fn single_letter_generic_is_clean() {
  clear_errors();

  let checker = Checker::new();

  checker.check_type_name("T", Span::ZERO, 0);
  checker.check_type_name("$T", Span::ZERO, 0);

  assert!(drained_rename().is_none());
}

#[test]
fn constant_name_must_be_screaming_case() {
  clear_errors();

  let checker = Checker::new();

  checker.check_constant_name("max_size", Span::ZERO, 0);

  let (kind, rename) = drained_rename().unwrap();

  assert_eq!(kind, ErrorKind::NonScreamingCaseName);
  assert_eq!(rename, "MAX_SIZE");
}

#[test]
fn screaming_case_constant_is_clean() {
  clear_errors();

  let checker = Checker::new();

  checker.check_constant_name("MAX_SIZE", Span::ZERO, 0);
  checker.check_constant_name("PI", Span::ZERO, 0);

  assert!(drained_rename().is_none());
}

#[test]
fn binding_name_must_be_snake_case() {
  clear_errors();

  let checker = Checker::new();

  checker.check_binding_name("myCount", Span::ZERO, 0);

  let (kind, rename) = drained_rename().unwrap();

  assert_eq!(kind, ErrorKind::NonSnakeCaseName);
  assert_eq!(rename, "my_count");
}

#[test]
fn snake_case_binding_is_clean() {
  clear_errors();

  let checker = Checker::new();

  checker.check_binding_name("my_count", Span::ZERO, 0);
  checker.check_binding_name("x", Span::ZERO, 0);

  assert!(drained_rename().is_none());
}

#[test]
fn digits_need_no_separator() {
  // inflector treats every digit as a word boundary (`r0` →
  // `r_0`); the swisskit predicates must not — `r0`, `MAX2`,
  // and `Vec2` are idiomatic in their conventions.
  clear_errors();

  let checker = Checker::new();

  checker.check_binding_name("r0", Span::ZERO, 0);
  checker.check_binding_name("grid2", Span::ZERO, 0);
  checker.check_constant_name("MAX2", Span::ZERO, 0);
  checker.check_type_name("Vec2", Span::ZERO, 0);

  assert!(drained_rename().is_none());
}

#[test]
fn underscore_prefix_opts_out() {
  clear_errors();

  let checker = Checker::new();

  checker.check_binding_name("_", Span::ZERO, 0);
  checker.check_binding_name("_unused", Span::ZERO, 0);

  assert!(drained_rename().is_none());
}

#[test]
fn naming_warnings_never_block_compilation() {
  assert_eq!(severity(ErrorKind::NonPascalCaseName), Severity::Warning);
  assert_eq!(severity(ErrorKind::NonScreamingCaseName), Severity::Warning);
  assert_eq!(severity(ErrorKind::NonSnakeCaseName), Severity::Warning);
}
