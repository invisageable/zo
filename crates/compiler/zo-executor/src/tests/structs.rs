use crate::tests::common::{
  assert_execution_error, assert_no_errors, assert_sir_structure,
};

use zo_error::ErrorKind;
use zo_sir::Insn;
use zo_ty::SelfKind;

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
  imu s := Span { lo = 0, hi = 10 };
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
  imu s: Span = Span { lo = 0, hi = 10 };
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
  let source = r#"ffi check(b: bool);
struct Span { lo: int, hi: int }
apply Span {
  fun new(lo: int, hi: int) -> Self {
    Self { lo = lo, hi = hi }
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

  assert_no_errors(source);
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
  imu p: Point = Point { x = 10, y = 20 };
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
  fun new() -> Self { Self { x = 0 } }
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
fn test_own_self_records_consume_kind() {
  // `own self` is a consuming receiver: it records
  // SelfKind::Consume and, because it moves rather than
  // mut-borrows, may be called on an `imu` binding without
  // an ImmutableVariable error (assert_sir_structure also
  // asserts the compile is error-free).
  assert_sir_structure(
    r#"struct Widget { x: int }
apply Widget {
  fun new() -> Self { Self { x = 0 } }
  fun consume(own self) -> int { self.x }
}
fun main() {
  imu w: Widget = Widget::new();
  imu got: int = w.consume();
}"#,
    |sir| {
      let has_consume = sir.iter().any(|i| {
        matches!(
          i,
          Insn::FunDef {
            self_kind: SelfKind::Consume,
            ..
          }
        )
      });

      assert!(has_consume, "own self should record SelfKind::Consume");
    },
  );
}

#[test]
fn test_own_self_allows_field_mutation() {
  // `own self` owns the receiver, so its body may mutate it.
  assert_sir_structure(
    r#"struct Builder { x: int }
apply Builder {
  fun new() -> Self { Self { x = 0 } }
  fun set_x(own self, v: int) { self.x = v; }
}
fun main() {
  imu b: Builder = Builder::new();
  b.set_x(7);
}"#,
    |sir| {
      let has_field_store =
        sir.iter().any(|i| matches!(i, Insn::FieldStore { .. }));

      assert!(has_field_store, "own self should permit field mutation");
    },
  );
}

#[test]
fn test_immutable_self_rejects_field_mutation() {
  assert_execution_error(
    r#"struct Counter { x: int }
apply Counter {
  fun new() -> Self { Self { x = 0 } }
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
    Self { w = w, h = h }
  }
  fun area(self) -> int {
    self.w
  }
}
fun main() {
  imu r: Rect = Rect::new(10, 20);
  imu a := r.area();
}"#;

  assert_no_errors(source);
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

#[test]
fn test_user_type_in_array_annotation() {
  // `[]Todo` with a user-defined element type used to fall
  // through `resolve_array_type`'s `is_ty()` filter and
  // report a spurious "Type mismatch" at the decl site.
  assert_sir_structure(
    r#"struct Todo { text: str, done: bool }

fun main() {
  mut xs: []Todo = [];
  xs.push(Todo { text = "a", done = false });
}"#,
    |sir| {
      let has_array =
        sir.iter().any(|i| matches!(i, Insn::ArrayLiteral { .. }));

      assert!(has_array, "expected ArrayLiteral for []Todo init");
    },
  );
}

#[test]
fn test_field_assign_outside_apply_lowers_to_field_store() {
  // `p.x = expr` was a silent no-op. Now it lowers to
  // `Insn::FieldStore` via `pending_field_assign`.
  assert_sir_structure(
    r#"struct Point { x: int, y: int }

fun main() {
  mut p: Point = Point { x = 1, y = 2 };
  p.x = 99;
}"#,
    |sir| {
      let has_field_store =
        sir.iter().any(|i| matches!(i, Insn::FieldStore { .. }));

      assert!(has_field_store, "expected FieldStore for `p.x = 99`");
    },
  );
}

#[test]
fn test_nested_field_assign_lowers_to_field_store() {
  // `a.b.c = expr` was a silent no-op: the receiver walk read
  // the token two back from `=`, an inner `Dot` rather than a
  // variable, so the mutability check failed and no store was
  // emitted. The walk now climbs to the root variable.
  assert_sir_structure(
    r#"struct Position { x: float, y: float }
struct Body { position: Position }

fun main() {
  mut body: Body = Body { position = Position { x = 1.0, y = 2.0 } };
  body.position.x = 7.0;
}"#,
    |sir| {
      let has_field_store =
        sir.iter().any(|i| matches!(i, Insn::FieldStore { .. }));

      assert!(
        has_field_store,
        "expected FieldStore for `body.position.x = 7.0`"
      );
    },
  );
}

#[test]
fn test_nested_field_compound_assign_lowers_to_field_store() {
  // `a.b.c op= expr` recomputed the field offset from the root
  // type, which only addresses one level deep, so a nested
  // target found no field and bailed. It now recovers the base
  // pointer + field index from the read and emits BinOp + Store.
  assert_sir_structure(
    r#"struct Position { x: float, y: float }
struct Body { position: Position }

fun main() {
  mut body: Body = Body { position = Position { x = 10.0, y = 2.0 } };
  body.position.x -= 3.0;
}"#,
    |sir| {
      let has_binop = sir.iter().any(|i| matches!(i, Insn::BinOp { .. }));
      let has_field_store =
        sir.iter().any(|i| matches!(i, Insn::FieldStore { .. }));

      assert!(has_binop, "compound assign must emit a BinOp");
      assert!(
        has_field_store,
        "expected FieldStore for `body.position.x -= 3.0`"
      );
    },
  );
}

#[test]
fn test_self_field_assign_lowers_to_field_store() {
  // `self.field = expr` inside an `apply` method must reach
  // `Insn::FieldStore` too. `self` lowers to `Param(0)`, not
  // `Local(SELF)`, so the receiver-name lookup pulls from
  // the parse tree (`Token::SelfLower`) rather than walking
  // the SIR for a `LoadSource::Local`.
  assert_sir_structure(
    r#"struct Flag { on: bool }

apply Flag {
  fun toggle(mut self) {
    self.on = !self.on;
  }
}

fun main() {
  mut f: Flag = Flag { on = false };
  f.toggle();
}"#,
    |sir| {
      let has_field_store =
        sir.iter().any(|i| matches!(i, Insn::FieldStore { .. }));

      assert!(has_field_store, "expected FieldStore inside `apply` method");
    },
  );
}

#[test]
fn test_derive_json_unsupported_field_reports_diagnostic() {
  // `Fn(...)` fields fall outside the derive synthesizer's
  // supported shape (primitives / nested structs / `[]T`).
  // Each direction (`to_json` / `from_json`) anchors a
  // `DeriveUnsupportedField` diagnostic on the offending
  // field's declaration span so the user can see which
  // field is blocking the derive.
  assert_execution_error(
    r#"
      struct Json {}

      %% serialize, deserialize.
      struct Bad {
        id: int,
        callback: Fn(int) -> int,
      }

      fun main() {}
    "#,
    ErrorKind::DeriveUnsupportedField,
  );
}

#[test]
fn test_chained_field_then_index_emits_sir() {
  // `m.verts[0]` — field access producing an array,
  // then indexing into it. Requires LBracket to
  // recognize Token::Dot as a value-producing
  // predecessor in the postorder tree.
  assert_sir_structure(
    r#"struct Vec2 { x: int, y: int }
struct Mesh { verts: []Vec2 }

fun main() {
  imu pts: []Vec2 = [
    Vec2 { x: 1, y: 2 },
  ];
  imu m := Mesh { verts: pts };
  imu v := m.verts[0];
}"#,
    |sir| {
      let has_tuple_index =
        sir.iter().any(|i| matches!(i, Insn::TupleIndex { .. }));
      let has_array_index =
        sir.iter().any(|i| matches!(i, Insn::ArrayIndex { .. }));

      assert!(
        has_tuple_index,
        "field access (m.verts) should emit TupleIndex"
      );
      assert!(
        has_array_index,
        "index after field (m.verts[0]) should emit ArrayIndex"
      );
    },
  );
}

#[test]
fn test_chained_field_index_field_emits_sir() {
  // `m.verts[0].x` — full chain: field, index, field.
  assert_sir_structure(
    r#"struct Vec2 { x: int, y: int }
struct Mesh { verts: []Vec2 }

fun main() {
  imu pts: []Vec2 = [
    Vec2 { x: 10, y: 20 },
  ];
  imu m := Mesh { verts: pts };
  imu v := m.verts[0].x;
}"#,
    |sir| {
      let tuple_indices = sir
        .iter()
        .filter(|i| matches!(i, Insn::TupleIndex { .. }))
        .count();
      let has_array_index =
        sir.iter().any(|i| matches!(i, Insn::ArrayIndex { .. }));

      assert!(
        tuple_indices >= 2,
        "expected >= 2 TupleIndex (m.verts + .x), got {}",
        tuple_indices
      );
      assert!(has_array_index, "expected ArrayIndex for [0]");
    },
  );
}

#[test]
fn test_struct_construct_colon_separator_rejected() {
  // Construction binds fields with `=`; `:` belongs to definitions.
  // A `:` separator used to slip through the shorthand branch and
  // emit garbage instead of a diagnostic.
  assert_execution_error(
    r#"struct Foo { a: int, b: int }
fun main() {
  imu x := Foo { a: 1, b: 2 };
}"#,
    ErrorKind::ExpectedAssignment,
  );
}
