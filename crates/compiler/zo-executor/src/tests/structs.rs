use crate::Executor;
use crate::tests::common::{assert_execution_error, assert_sir_structure};

use zo_error::ErrorKind;
use zo_interner::Interner;
use zo_parser::Parser;
use zo_reporter::collect_errors;
use zo_sir::Insn;
use zo_tokenizer::Tokenizer;
use zo_ty_checker::TyChecker;

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

  let (_, _, _) = executor.execute();
  let errors = collect_errors();

  assert!(
    errors.is_empty(),
    "apply instance method should not error: {:?}",
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>()
  );
}

// === PUB FIELDS ===

#[test]
fn test_struct_pub_fields() {
  assert_sir_structure(
    r#"struct Color {
  pub r: int,
  pub g: int,
  pub b: int,
}
fun main() {}"#,
    |sir| {
      let def = sir.iter().find(|i| matches!(i, Insn::StructDef { .. }));

      if let Some(Insn::StructDef { fields, .. }) = def {
        assert_eq!(fields.len(), 3, "Color has 3 fields");
      } else {
        panic!("expected StructDef for pub fields");
      }
    },
  );
}

#[test]
fn test_struct_mixed_pub_fields() {
  assert_sir_structure(
    r#"struct Mixed {
  pub name: str,
  age: int,
  pub active: bool,
}
fun main() {}"#,
    |sir| {
      let def = sir.iter().find(|i| matches!(i, Insn::StructDef { .. }));

      if let Some(Insn::StructDef { fields, .. }) = def {
        assert_eq!(fields.len(), 3);
      } else {
        panic!("expected StructDef for mixed pub fields");
      }
    },
  );
}

// === FIELD ACCESS (TupleIndex) ===

#[test]
fn test_struct_field_access_emits_tuple_index() {
  assert_sir_structure(
    r#"struct Point { x: int, y: int }
fun main() {
  imu p: Point = Point { x: 10, y: 20 };
  check@eq(p.x, 10);
}"#,
    |sir| {
      let has_tuple_index =
        sir.iter().any(|i| matches!(i, Insn::TupleIndex { .. }));

      assert!(has_tuple_index, "field access should emit TupleIndex");
    },
  );
}

// === MUT SELF ===

#[test]
fn test_mut_self_allows_field_mutation() {
  assert_sir_structure(
    r#"struct Counter { x: int }
apply Counter {
  fun new() -> Self { Self { x: 0 } }
  fun incr(mut self) { self.x += 1; }
}
fun main() {
  imu c: Counter = Counter::new();
  c.incr();
}"#,
    |sir| {
      let has_field_store =
        sir.iter().any(|i| matches!(i, Insn::FieldStore { .. }));

      assert!(
        has_field_store,
        "mut self compound assign should emit FieldStore"
      );
    },
  );
}

#[test]
fn test_immutable_self_rejects_field_mutation() {
  assert_execution_error(
    r#"struct Counter { x: int }
apply Counter {
  fun new() -> Self { Self { x: 0 } }
  fun incr(self) { self.x += 1; }
}
fun main() {}"#,
    ErrorKind::ImmutableVariable,
  );
}

// === SELF VALUE IN METHOD BODY ===

#[test]
fn test_self_lower_emits_load() {
  assert_sir_structure(
    r#"struct Foo { x: int }
apply Foo {
  fun get(self) -> int { self.x }
}
fun main() {}"#,
    |sir| {
      // Inside the method body, self should produce a
      // Load from Param(0).
      let has_self_load = sir.iter().any(|i| {
        matches!(
          i,
          Insn::Load {
            src: zo_sir::LoadSource::Param(0),
            ..
          }
        )
      });

      assert!(
        has_self_load,
        "self in method body should emit Load Param(0)"
      );
    },
  );
}

// === APPLY STATIC + INSTANCE END-TO-END ===

#[test]
fn test_apply_static_and_instance_no_errors() {
  let source = r#"struct Rect { w: int, h: int }
apply Rect {
  fun new(w: int, h: int) -> Self {
    Self { w: w, h: h }
  }
  fun area(self) -> int {
    self.w
  }
}
fun main() {
  imu r: Rect = Rect::new(10, 20);
  imu a := r.area();
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

  let (_, _, _) = executor.execute();
  let errors = collect_errors();

  assert!(
    errors.is_empty(),
    "apply static+instance should not error: {:?}",
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>()
  );
}

// === MUT PARAM (NON-SELF) ===

#[test]
fn test_mut_param_allows_reassign() {
  assert_sir_structure(
    r#"fun double(mut x: int) -> int {
  x += x;
  x
}
fun main() {}"#,
    |sir| {
      // mut x should allow compound assignment — no
      // ImmutableVariable error.
      let has_store = sir.iter().any(|i| matches!(i, Insn::Store { .. }));

      assert!(has_store, "mut param should allow Store");
    },
  );
}

#[test]
fn test_immutable_param_rejects_reassign() {
  assert_execution_error(
    r#"fun double(x: int) -> int {
  x += x;
  x
}
fun main() {}"#,
    ErrorKind::ImmutableVariable,
  );
}
