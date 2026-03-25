use crate::tests::common::assert_sir_structure;

use zo_sir::Insn;

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
