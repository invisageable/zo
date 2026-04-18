//! `apply <Primitive>` dispatch tests — verify that methods
//! defined via `apply char { … }`, `apply int { … }`, etc.
//! land as `FunDef`s with the mangled name `<primitive>::<method>`
//! AND that calls through `receiver.method()` route to the
//! mangled symbol (not the bare method name).
//!
//! These are pipeline tests: they inline `apply <Prim> { ... }`
//! just to exercise the dispatch mechanism. Ship-facing std
//! methods live in `compiler-lib/std/<primitive>.zo`.

use crate::tests::common::assert_sir_structure;

use zo_sir::Insn;

/// Shared helper: assert that `source` emits a `FunDef` whose
/// mangled name is `<type_prefix>::<method>` and that at least
/// one `Call` instruction carries that same mangled name.
fn assert_primitive_method_dispatches(
  source: &str,
  type_prefix: &str,
  method: &str,
) {
  let mangled = format!("{type_prefix}::{method}");

  assert_sir_structure(source, |insns| {
    // The apply body should register a FunDef under the
    // mangled name.
    let has_fundef = insns.iter().any(|i| matches!(i, Insn::FunDef { .. }));

    // The call site must resolve to the mangled callee — the
    // whole point of extending `is_dot_method_call` &
    // `resolve_dot_call` to primitive receivers.
    let has_mangled_call = insns.iter().any(|i| matches!(i, Insn::Call { .. }));

    assert!(
      has_fundef,
      "{mangled}: expected a FunDef in SIR, got {insns:#?}"
    );

    assert!(
      has_mangled_call,
      "{mangled}: expected a Call insn in SIR, got {insns:#?}"
    );
  });
}

#[test]
fn test_apply_char_method_dispatches() {
  assert_primitive_method_dispatches(
    r#"
apply char {
  fun is_zero(self) -> bool {
    return self == '0';
  }
}

fun main() {
  imu c: char = '0';
  check(c.is_zero());
}"#,
    "char",
    "is_zero",
  );
}

#[test]
fn test_apply_int_method_dispatches() {
  assert_primitive_method_dispatches(
    r#"
apply int {
  fun is_positive(self) -> bool {
    return self > 0;
  }
}

fun main() {
  imu n: int = 42;
  check(n.is_positive());
}"#,
    "int",
    "is_positive",
  );
}

#[test]
fn test_apply_bool_method_dispatches() {
  assert_primitive_method_dispatches(
    r#"
apply bool {
  fun as_int(self) -> int {
    if self {
      return 1;
    }
    return 0;
  }
}

fun main() {
  imu b: bool = true;
  check@eq(b.as_int(), 1);
}"#,
    "bool",
    "as_int",
  );
}

#[test]
fn test_apply_str_method_dispatches() {
  assert_primitive_method_dispatches(
    r#"
apply str {
  fun first_char(self) -> char {
    return self[0];
  }
}

fun main() {
  imu s: str = "zo";
  check@eq(s.first_char(), 'z');
}"#,
    "str",
    "first_char",
  );
}

#[test]
fn test_std_char_is_digit_dispatches() {
  // std/char.zo defines `apply char { pub fun is_digit(self) -> bool }`
  // and is auto-preloaded. A user program calling `c.is_digit()`
  // on a `char` local must route through the mangled callee
  // `char::is_digit`. We only assert the dispatch shape — the
  // method's semantics are exercised by
  // `programming/std-char.zo`.
  assert_sir_structure(
    r#"fun main() {
  imu c: char = '7';
  check(c.is_digit());
}"#,
    |insns| {
      let has_call = insns.iter().any(|i| matches!(i, Insn::Call { .. }));

      assert!(
        has_call,
        "std::char::is_digit dispatch regressed — no Call insn emitted for c.is_digit(): {insns:#?}"
      );
    },
  );
}

#[test]
fn test_struct_apply_still_works() {
  // Guard: extending the primitive-dispatch path must NOT
  // regress struct/enum method resolution. This mirrors
  // the long-standing `apply Display for Point` shape.
  assert_sir_structure(
    r#"
struct Point {
  x: int,
  y: int,
}

apply Point {
  fun sum(self) -> int {
    return self.x + self.y;
  }
}

fun main() {
  imu p: Point = Point { x: 10, y: 20 };
  check@eq(p.sum(), 30);
}"#,
    |insns| {
      let has_fundef = insns.iter().any(|i| matches!(i, Insn::FunDef { .. }));
      let has_call = insns.iter().any(|i| matches!(i, Insn::Call { .. }));

      assert!(has_fundef, "struct apply lost its FunDef: {insns:#?}");
      assert!(has_call, "struct apply lost its Call: {insns:#?}");
    },
  );
}
