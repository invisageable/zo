use crate::Executor;
use crate::tests::common::assert_sir_structure;

use zo_error::ErrorKind;
use zo_interner::Interner;
use zo_parser::Parser;
use zo_reporter::collect_errors;
use zo_sir::Insn;
use zo_tokenizer::Tokenizer;
use zo_ty_checker::TyChecker;

#[test]
fn test_type_alias_basic() {
  assert_sir_structure(
    r#"type Idx = int;

fun main() {
  imu x: Idx = 42;
  x
}"#,
    |sir| {
      // The alias should resolve Idx -> int, producing
      // a VarDef with the int type (not an error/unit).
      let has_var = sir.iter().any(|i| matches!(i, Insn::VarDef { .. }));

      assert!(has_var, "expected VarDef instruction");
    },
  );
}

#[test]
fn test_type_alias_chain() {
  assert_sir_structure(
    r#"type A = int;
type B = A;

fun main() {
  imu x: B = 42;
  x
}"#,
    |sir| {
      let has_var = sir.iter().any(|i| matches!(i, Insn::VarDef { .. }));

      assert!(has_var, "expected VarDef instruction");
    },
  );
}

#[test]
fn test_type_alias_in_fn_signature() {
  assert_sir_structure(
    r#"type Idx = int;

fun add(a: Idx, b: Idx) -> Idx {
  a + b
}

fun main() {
  add(1, 2)
}"#,
    |sir| {
      let has_fun = sir.iter().any(|i| matches!(i, Insn::FunDef { .. }));

      assert!(has_fun, "expected FunDef instruction");
    },
  );
}

#[test]
fn test_type_alias_pub() {
  assert_sir_structure(
    r#"pub type Idx = int;

fun main() {
  imu x: Idx = 42;
  x
}"#,
    |sir| {
      let has_var = sir.iter().any(|i| matches!(i, Insn::VarDef { .. }));

      assert!(has_var, "expected VarDef instruction");
    },
  );
}

#[test]
fn test_group_type_alias() {
  assert_sir_structure(
    r#"group type Idx = int
  and Num = float
;

fun main() {
  imu x: Idx = 42;
  x
}"#,
    |sir| {
      let has_var = sir.iter().any(|i| matches!(i, Insn::VarDef { .. }));

      assert!(has_var, "expected VarDef instruction");
    },
  );
}

// === GENERIC TYPE ALIAS — REGRESSIONS FOR `fix(zo): make
//     generic type aliases resolve their bodies` ===
//
// Before the fix, `execute_type_alias` never set up the
// `<$T>` type-param scope, so `$T` resolved to no known
// name and the alias body fell through to `unit`. Every
// use site (`Id<int> = 42`, `Pair<int> = (3, 7)`,
// `Grid<int> = [..]`) reported a spurious `TypeMismatch`
// because the registered alias was `()` rather than the
// real shape.

/// Boilerplate-free runner: compile, drain thread-local
/// reporter errors, return them. The existing
/// `assert_sir_structure` helper swallows reporter
/// errors silently — these tests need to assert their
/// absence as a positive guarantee.
fn run_collect_errors(source: &str) -> Vec<zo_error::Error> {
  let mut interner = Interner::new();
  let tokenizer = Tokenizer::new(source, &mut interner);
  let tokenization = tokenizer.tokenize();
  let parser = Parser::new(&tokenization, source);
  let parsing = parser.parse();
  let mut ty_checker = TyChecker::new();

  let executor = Executor::new(
    &parsing.tree,
    &mut interner,
    &tokenization.literals,
    &mut ty_checker,
  );

  executor.execute();

  collect_errors()
}

#[test]
fn test_generic_alias_id_int() {
  // Pre-fix: `Id<int> = 42` produced TypeMismatch
  // because the alias body resolved to unit.
  let errors = run_collect_errors(
    r#"type Id<$T> = $T;

fun main() {
  imu x: Id<int> = 42;
}"#,
  );

  assert!(
    !errors
      .iter()
      .any(|e| matches!(e.kind(), ErrorKind::TypeMismatch)),
    "Id<int> = 42 must not report TypeMismatch; got: {:?}",
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>()
  );
}

#[test]
fn test_generic_alias_pair_int_tuple_body() {
  // Pre-fix: tuple body `($T, $T)` collapsed to `((),
  // ())`, so `Pair<int> = (3, 7)` mismatched.
  let errors = run_collect_errors(
    r#"type Pair<$T> = ($T, $T);

fun main() {
  imu pos: Pair<int> = (3, 7);
}"#,
  );

  assert!(
    !errors
      .iter()
      .any(|e| matches!(e.kind(), ErrorKind::TypeMismatch)),
    "Pair<int> = (3, 7) must not report TypeMismatch; got: {:?}",
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>()
  );
}

#[test]
fn test_generic_alias_grid_int_array_body() {
  // Pre-fix: `resolve_array_type` rejected `Dollar`, so
  // `Grid<$T> = []$T` had no element type and `Grid<int>
  // = [1, 2, 3]` mismatched.
  let errors = run_collect_errors(
    r#"type Grid<$T> = []$T;

fun main() {
  imu row: Grid<int> = [10, 20, 30];
}"#,
  );

  assert!(
    !errors
      .iter()
      .any(|e| matches!(e.kind(), ErrorKind::TypeMismatch)),
    "Grid<int> = [...] must not report TypeMismatch; got: {:?}",
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>()
  );
}

#[test]
fn test_generic_alias_two_instantiations_no_leak() {
  // The alias body's inference vars are SHARED across
  // every `lookup_ty_alias` call. If the use-site path
  // doesn't substitute them with fresh args per use,
  // `Pair<int>` binds the body var to `int`, and the
  // following `Pair<str>` then unifies `int` with `str`
  // and reports TypeMismatch. The fix substitutes per
  // use, isolating instantiations.
  let errors = run_collect_errors(
    r#"type Pair<$T> = ($T, $T);

fun main() {
  imu pi: Pair<int> = (3, 7);
  imu ps: Pair<str> = ("a", "b");
}"#,
  );

  assert!(
    !errors
      .iter()
      .any(|e| matches!(e.kind(), ErrorKind::TypeMismatch)),
    "two distinct Pair instantiations must not interfere; got: {:?}",
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>()
  );
}

#[test]
fn test_generic_alias_tuple_body_field_access() {
  // Downstream uses of the substituted alias must work:
  // `pos.0` should type-check as the substituted element.
  // Regression: a stale unit-tuple body would have made
  // `pos.0` resolve to `()` and any subsequent `int`
  // annotation fail.
  let errors = run_collect_errors(
    r#"type Pair<$T> = ($T, $T);

fun main() {
  imu pos: Pair<int> = (3, 7);
  imu px: int = pos.0;
  imu py: int = pos.1;
}"#,
  );

  assert!(
    !errors
      .iter()
      .any(|e| matches!(e.kind(), ErrorKind::TypeMismatch)),
    "Pair<int> tuple field access must type-check; got: {:?}",
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>()
  );
}

#[test]
fn test_generic_alias_array_body_index_access() {
  // Mirror of the tuple test for array bodies: indexing
  // into a `Grid<int>` must yield `int`.
  let errors = run_collect_errors(
    r#"type Grid<$T> = []$T;

fun main() {
  imu row: Grid<int> = [10, 20, 30];
  imu first: int = row[0];
}"#,
  );

  assert!(
    !errors
      .iter()
      .any(|e| matches!(e.kind(), ErrorKind::TypeMismatch)),
    "Grid<int>[0] must type-check as int; got: {:?}",
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>()
  );
}
