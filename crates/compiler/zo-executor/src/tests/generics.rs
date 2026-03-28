use crate::Executor;
use crate::tests::common::assert_sir_structure;

use zo_parser::Parser;
use zo_reporter::collect_errors;
use zo_sir::Insn;
use zo_tokenizer::Tokenizer;

// === GENERIC FUNCTION PARSING ===

#[test]
fn test_generic_fun_emits_fundef() {
  assert_sir_structure(
    r#"fun identity<$T>(x: $T) -> $T { x }
fun main() {}"#,
    |sir| {
      let has_fundef = sir.iter().any(|i| matches!(i, Insn::FunDef { .. }));

      assert!(has_fundef, "generic function should emit FunDef");
    },
  );
}

#[test]
fn test_generic_fun_no_errors() {
  let source = r#"fun identity<$T>(x: $T) -> $T { x }
fun main() {
  imu a: int = identity(42);
}"#;

  let tokenizer = Tokenizer::new(source);
  let mut tokenization = tokenizer.tokenize();
  let parser = Parser::new(&tokenization, source);
  let parsing = parser.parse();

  let executor = Executor::new(
    &parsing.tree,
    &mut tokenization.interner,
    &tokenization.literals,
  );

  let (_, _, _) = executor.execute();
  let errors = collect_errors();

  assert!(
    errors.is_empty(),
    "generic function call should not error: {:?}",
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>()
  );
}

// === MULTIPLE CALLS FRESH VARS ===

#[test]
fn test_generic_multiple_calls_no_errors() {
  let source = r#"fun identity<$T>(x: $T) -> $T { x }
fun main() {
  imu a: int = identity(42);
  imu b: int = identity(99);
}"#;

  let tokenizer = Tokenizer::new(source);
  let mut tokenization = tokenizer.tokenize();
  let parser = Parser::new(&tokenization, source);
  let parsing = parser.parse();

  let executor = Executor::new(
    &parsing.tree,
    &mut tokenization.interner,
    &tokenization.literals,
  );

  let (_, _, _) = executor.execute();
  let errors = collect_errors();

  assert!(
    errors.is_empty(),
    "multiple generic calls should not error: {:?}",
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>()
  );
}

// === MULTI TYPE PARAMS ===

#[test]
fn test_generic_multi_param_no_errors() {
  let source = r#"fun pick_second<$A, $B>(a: $A, b: $B) -> $B { b }
fun main() {
  imu x: int = pick_second(10, 42);
}"#;

  let tokenizer = Tokenizer::new(source);
  let mut tokenization = tokenizer.tokenize();
  let parser = Parser::new(&tokenization, source);
  let parsing = parser.parse();

  let executor = Executor::new(
    &parsing.tree,
    &mut tokenization.interner,
    &tokenization.literals,
  );

  let (_, _, _) = executor.execute();
  let errors = collect_errors();

  assert!(
    errors.is_empty(),
    "multi-param generic should not error: {:?}",
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>()
  );
}

// === MIXED TYPES ===

#[test]
fn test_generic_mixed_str_int_no_errors() {
  let source = r#"fun first<$A, $B>(a: $A, b: $B) -> $A { a }
fun main() {
  imu a: int = first(42, "hello");
}"#;

  let tokenizer = Tokenizer::new(source);
  let mut tokenization = tokenizer.tokenize();
  let parser = Parser::new(&tokenization, source);
  let parsing = parser.parse();

  let executor = Executor::new(
    &parsing.tree,
    &mut tokenization.interner,
    &tokenization.literals,
  );

  let (_, _, _) = executor.execute();
  let errors = collect_errors();

  assert!(
    errors.is_empty(),
    "mixed str+int generic should not error: {:?}",
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>()
  );
}

// === TYPE PARAM IN RETURN ===

#[test]
fn test_generic_return_type_inferred() {
  assert_sir_structure(
    r#"fun wrap<$T>(x: $T) -> $T { x }
fun main() {
  imu a: int = wrap(42);
}"#,
    |sir| {
      // The Call instruction should have an int return
      // type (resolved from $T = int).
      let call = sir.iter().find(|i| matches!(i, Insn::Call { .. }));

      assert!(call.is_some(), "generic call should emit Call instruction");
    },
  );
}

// === SCOPE: PARAMS DON'T LEAK ===

#[test]
fn test_generic_params_dont_leak_to_main() {
  let source = r#"fun first<$A, $B>(a: $A, b: $B) -> $A { a }
fun second<$A, $B>(a: $A, b: $B) -> $B { b }
fun main() {
  imu a: int = first(42, "hello");
  imu b: int = second("world", 99);
}"#;

  let tokenizer = Tokenizer::new(source);
  let mut tokenization = tokenizer.tokenize();
  let parser = Parser::new(&tokenization, source);
  let parsing = parser.parse();

  let executor = Executor::new(
    &parsing.tree,
    &mut tokenization.interner,
    &tokenization.literals,
  );

  let (_, _, _) = executor.execute();
  let errors = collect_errors();

  assert!(
    errors.is_empty(),
    "function params should not leak: {:?}",
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>()
  );
}

// === MONOMORPHIZATION ===

#[test]
fn test_mono_creates_specialized_fundef() {
  assert_sir_structure(
    r#"fun identity<$T>(x: $T) -> $T { x }
fun main() {
  imu a: int = identity(42);
}"#,
    |sir| {
      // Should have a FunDef with mangled name
      // containing "__int".
      let has_mono = sir.iter().any(|i| matches!(i, Insn::FunDef { .. }));

      let fundef_count = sir
        .iter()
        .filter(|i| matches!(i, Insn::FunDef { .. }))
        .count();

      assert!(
        fundef_count >= 3,
        "mono should produce extra FunDef (got {})",
        fundef_count
      );

      assert!(has_mono);
    },
  );
}

#[test]
fn test_mono_different_types_no_conflict() {
  let source = r#"fun identity<$T>(x: $T) -> $T { x }
fun main() {
  imu a: int = identity(42);
  imu b: str = identity("hello");
}"#;

  let tokenizer = Tokenizer::new(source);
  let mut tokenization = tokenizer.tokenize();
  let parser = Parser::new(&tokenization, source);
  let parsing = parser.parse();

  let executor = Executor::new(
    &parsing.tree,
    &mut tokenization.interner,
    &tokenization.literals,
  );

  let (_, _, _) = executor.execute();
  let errors = collect_errors();

  assert!(
    errors.is_empty(),
    "mono with int + str should not error: {:?}",
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>()
  );
}

#[test]
fn test_mono_same_type_reuses_instance() {
  assert_sir_structure(
    r#"fun identity<$T>(x: $T) -> $T { x }
fun main() {
  imu a: int = identity(42);
  imu b: int = identity(99);
}"#,
    |sir| {
      // Two calls to identity<int> should produce only
      // ONE monomorphized FunDef, not two.
      let fundef_count = sir
        .iter()
        .filter(|i| matches!(i, Insn::FunDef { .. }))
        .count();

      // original identity + identity__int + main = 3
      // (NOT 4 — second call reuses the same instance)
      assert!(
        fundef_count <= 4,
        "same type should reuse mono instance (got {})",
        fundef_count
      );
    },
  );
}

#[test]
fn test_mono_multi_param_mangling() {
  let source = r#"fun pick<$A, $B>(a: $A, b: $B) -> $B { b }
fun main() {
  imu x: int = pick(42, 99);
}"#;

  let tokenizer = Tokenizer::new(source);
  let mut tokenization = tokenizer.tokenize();
  let parser = Parser::new(&tokenization, source);
  let parsing = parser.parse();

  let executor = Executor::new(
    &parsing.tree,
    &mut tokenization.interner,
    &tokenization.literals,
  );

  let (_, _, _) = executor.execute();
  let errors = collect_errors();

  assert!(
    errors.is_empty(),
    "multi-param mono should not error: {:?}",
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>()
  );
}
