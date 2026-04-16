use crate::Executor;
use crate::tests::common::{assert_execution_error, assert_sir_structure};

use zo_error::ErrorKind;
use zo_interner::Interner;
use zo_parser::Parser;
use zo_reporter::collect_errors;
use zo_sir::Insn;
use zo_tokenizer::Tokenizer;
use zo_ty_checker::TyChecker;

// === VAL BASIC ===

#[test]
fn test_val_emits_const_def() {
  assert_sir_structure(
    r#"fun main() {
  val X: int = 42;
}"#,
    |sir| {
      let has_const_def =
        sir.iter().any(|i| matches!(i, Insn::ConstDef { .. }));

      assert!(has_const_def, "val should emit ConstDef instruction");
    },
  );
}

#[test]
fn test_val_no_store_emitted() {
  assert_sir_structure(
    r#"fun main() {
  val X: int = 42;
}"#,
    |sir| {
      let has_store = sir
        .iter()
        .any(|i| matches!(i, Insn::Store { name, .. } if name.0 != 0));

      // val should NOT emit Store — constants are inlined.
      assert!(!has_store, "val should not emit Store instruction");
    },
  );
}

#[test]
fn test_val_constant_inlined_at_use() {
  assert_sir_structure(
    r#"ext show(x: int);
fun main() {
  val X: int = 42;
  show(X);
}"#,
    |sir| {
      // The reference to X should re-emit a ConstInt
      // (inline), not a Load.
      let const_ints: Vec<_> = sir
        .iter()
        .filter(|i| matches!(i, Insn::ConstInt { value: 42, .. }))
        .collect();

      // One from the val init, one from the inline
      // reference.
      assert!(
        const_ints.len() >= 2,
        "val reference should re-emit ConstInt (got {})",
        const_ints.len()
      );
    },
  );
}

// === VAL ERRORS ===

#[test]
fn test_val_rejects_colon_eq() {
  assert_execution_error(
    r#"val x := 42;
fun main() {}"#,
    ErrorKind::ValRequiresTypeAnnotation,
  );
}

#[test]
fn test_val_rejects_string_colon_eq() {
  assert_execution_error(
    r#"val name := "hello";
fun main() {}"#,
    ErrorKind::ValRequiresTypeAnnotation,
  );
}

#[test]
fn test_val_local_rejects_colon_eq() {
  assert_execution_error(
    r#"fun main() {
  val x := 42;
}"#,
    ErrorKind::ValRequiresTypeAnnotation,
  );
}

// === VAL IN FUNCTION ===

#[test]
fn test_val_in_function_no_errors() {
  let source = r#"fun main() {
  val X: int = 42;
  val Y: int = 10;
}"#;

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

  let (_, _, _, _) = executor.execute();
  let errors = collect_errors();

  assert!(
    errors.is_empty(),
    "val in function should not error: {:?}",
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>()
  );
}

// === VAL GLOBAL ===

#[test]
fn test_val_global_no_errors() {
  let source = r#"val X: int = 42;
fun main() {}"#;

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

  let (_, _, _, _) = executor.execute();
  let errors = collect_errors();

  assert!(
    errors.is_empty(),
    "global val should not error: {:?}",
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>()
  );
}

#[test]
fn test_val_global_sir_has_no_module_level_constint() {
  assert_sir_structure(
    r#"val X: int = 42;
fun main() {}"#,
    |sir| {
      // Module-level ConstInt should be stripped — only
      // ConstInts inside the function body should remain.
      let pre_fundef_constint = sir
        .iter()
        .take_while(|i| !matches!(i, Insn::FunDef { .. }))
        .any(|i| matches!(i, Insn::ConstInt { .. }));

      assert!(
        !pre_fundef_constint,
        "module-level ConstInt should be stripped for val"
      );
    },
  );
}

// === VAL MULTIPLE TYPES ===

#[test]
fn test_val_bool_no_errors() {
  let source = r#"fun main() {
  val FLAG: bool = true;
}"#;

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

  let (_, _, _, _) = executor.execute();
  let errors = collect_errors();

  assert!(
    errors.is_empty(),
    "val bool should not error: {:?}",
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>()
  );
}

#[test]
fn test_val_str_no_errors() {
  let source = r#"fun main() {
  val GREETING: str = "hello";
}"#;

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

  let (_, _, _, _) = executor.execute();
  let errors = collect_errors();

  assert!(
    errors.is_empty(),
    "val str should not error: {:?}",
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>()
  );
}

// === VAL PUBNESS ===

#[test]
fn test_val_pub_no_errors() {
  let source = r#"pub val PI: int = 3;
fun main() {}"#;

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

  let (_, _, _, _) = executor.execute();
  let errors = collect_errors();

  assert!(
    errors.is_empty(),
    "pub val should not error: {:?}",
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>()
  );
}
