//! Shared test helpers for `zo-ty-checker`.
//!
//! These operate at the `TyChecker` level directly — no
//! tokenize/parse pipeline needed.

use crate::TyChecker;

use zo_error::ErrorKind;
use zo_reporter::collect_errors;
use zo_span::Span;
use zo_ty::TyId;

/// Assert that unifying `a` and `b` produces the expected error.
pub(crate) fn assert_unify_error(
  checker: &mut TyChecker,
  a: TyId,
  b: TyId,
  expected: ErrorKind,
) {
  // Drain any stale errors first.
  let _ = collect_errors();

  let result = checker.unify(a, b, Span::ZERO);

  assert!(
    result.is_none(),
    "Expected unification to fail with {:?}, but it succeeded",
    expected,
  );

  let errors = collect_errors();

  assert!(
    errors.iter().any(|e| e.kind() == expected),
    "Expected error {:?}, but got: {:?}",
    expected,
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>()
  );
}

/// Assert that unifying `a` and `b` succeeds.
pub(crate) fn assert_unify_ok(
  checker: &mut TyChecker,
  a: TyId,
  b: TyId,
) -> TyId {
  // Drain any stale errors first.
  let _ = collect_errors();

  let result = checker.unify(a, b, Span::ZERO);

  let errors = collect_errors();

  assert!(
    errors.is_empty(),
    "Expected no errors, but got: {:?}",
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>()
  );

  result.expect("Expected unification to succeed")
}

/// Assert that looking up a variable produces the expected error.
pub(crate) fn assert_lookup_error(
  checker: &mut TyChecker,
  name: zo_interner::Symbol,
  expected: ErrorKind,
) {
  let _ = collect_errors();

  let result = checker.infer_var(name, Span::ZERO);

  assert!(
    result.is_none(),
    "Expected lookup to fail with {:?}, but it succeeded",
    expected,
  );

  let errors = collect_errors();

  assert!(
    errors.iter().any(|e| e.kind() == expected),
    "Expected error {:?}, but got: {:?}",
    expected,
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>()
  );
}
