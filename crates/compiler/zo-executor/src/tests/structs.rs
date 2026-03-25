use crate::Executor;
use crate::tests::common::assert_sir_structure;

use zo_parser::Parser;
use zo_reporter::collect_errors;
use zo_sir::Insn;
use zo_tokenizer::Tokenizer;

#[test]
fn test_struct_empty() {
  assert_sir_structure(
    r#"struct Foo {}
fun main() {}"#,
    |sir| {
      let def = sir.iter().find(|i| matches!(i, Insn::StructDef { .. }));

      assert!(def.is_some(), "expected StructDef");

      if let Some(Insn::StructDef { fields, .. }) = def {
        assert!(fields.is_empty(), "empty struct has no fields");
      }
    },
  );
}

#[test]
fn test_struct_fields() {
  assert_sir_structure(
    r#"struct Span {
  lo: int,
  hi: int,
}
fun main() {}"#,
    |sir| {
      let def = sir.iter().find(|i| matches!(i, Insn::StructDef { .. }));

      if let Some(Insn::StructDef { fields, .. }) = def {
        assert_eq!(fields.len(), 2, "Span has 2 fields");
      } else {
        panic!("expected StructDef");
      }
    },
  );
}

#[test]
fn test_struct_default_value() {
  assert_sir_structure(
    r#"struct Bar {
  x: int = 0,
  y: int = 0,
}
fun main() {}"#,
    |sir| {
      let def = sir.iter().find(|i| matches!(i, Insn::StructDef { .. }));

      if let Some(Insn::StructDef { fields, .. }) = def {
        assert_eq!(fields.len(), 2);
        // Both fields have defaults.
        assert!(fields[0].2, "x has default");
        assert!(fields[1].2, "y has default");
      } else {
        panic!("expected StructDef");
      }
    },
  );
}

#[test]
fn test_struct_mixed_defaults() {
  assert_sir_structure(
    r#"struct Mixed {
  a: int,
  b: int = 42,
}
fun main() {}"#,
    |sir| {
      let def = sir.iter().find(|i| matches!(i, Insn::StructDef { .. }));

      if let Some(Insn::StructDef { fields, .. }) = def {
        assert_eq!(fields.len(), 2);
        assert!(!fields[0].2, "a has no default");
        assert!(fields[1].2, "b has default");
      } else {
        panic!("expected StructDef");
      }
    },
  );
}

// === STRUCT CONSTRUCTION ===

#[test]
fn test_struct_construct() {
  assert_sir_structure(
    r#"struct Span { lo: int, hi: int }
fun main() {
  imu s := Span { lo: 0, hi: 10 };
}"#,
    |sir| {
      let construct = sir
        .iter()
        .find(|i| matches!(i, Insn::StructConstruct { .. }));

      assert!(construct.is_some(), "expected StructConstruct");

      if let Some(Insn::StructConstruct { fields, .. }) = construct {
        assert_eq!(fields.len(), 2, "Span has 2 fields");
      }
    },
  );
}

#[test]
fn test_struct_shorthand_fields() {
  assert_sir_structure(
    r#"struct Span { lo: int, hi: int }
fun make(lo: int, hi: int) -> int {
  imu s := Span { lo, hi };
  0
}"#,
    |sir| {
      let construct = sir
        .iter()
        .find(|i| matches!(i, Insn::StructConstruct { .. }));

      assert!(
        construct.is_some(),
        "expected StructConstruct with shorthand fields"
      );

      if let Some(Insn::StructConstruct { fields, .. }) = construct {
        assert_eq!(fields.len(), 2);
      }
    },
  );
}

#[test]
fn test_struct_construct_with_annotation() {
  // Explicit type annotation must unify with init type.
  assert_sir_structure(
    r#"struct Span { lo: int, hi: int }
fun main() {
  imu s: Span = Span { lo: 0, hi: 10 };
}"#,
    |sir| {
      let construct = sir
        .iter()
        .find(|i| matches!(i, Insn::StructConstruct { .. }));

      assert!(
        construct.is_some(),
        "expected StructConstruct with type annotation"
      );
    },
  );
}

#[test]
fn test_apply_instance_method() {
  let source = r#"ext check(b: bool);
struct Span { lo: int, hi: int }
apply Span {
  fun new(lo: int, hi: int) -> Self {
    Self { lo: lo, hi: hi }
  }
  fun sum(self) -> int {
    self.lo + self.hi
  }
}
fun main() {
  imu s := Span::new(3, 7);
  imu n := s.sum();
  check@eq(n, 10);
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
    "apply instance method should not error: {:?}",
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>()
  );
}
