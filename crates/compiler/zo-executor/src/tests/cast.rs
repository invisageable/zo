use crate::tests::common::{assert_sir_structure, execute_raw};

use zo_sir::Insn;

#[test]
fn cast_int_to_char_emits_cast_insn() {
  let (sir, _) = execute_raw(
    r#"fun main() {
  imu c: char = 72 as char;
  showln(c);
}"#,
  );

  let has_cast = sir.iter().any(|i| matches!(i, Insn::Cast { .. }));

  assert!(has_cast, "expected Cast instruction in SIR");

  // Cast should have from_ty=int, to_ty=char.
  if let Some(Insn::Cast { from_ty, to_ty, .. }) =
    sir.iter().find(|i| matches!(i, Insn::Cast { .. }))
  {
    // int = TyId(8), char = TyId(3).
    assert_eq!(from_ty.0, 8, "expected from_ty=8 (int), got {}", from_ty.0);
    assert_eq!(to_ty.0, 3, "expected to_ty=3 (char), got {}", to_ty.0);
  }
}

#[test]
fn cast_char_to_int_emits_cast_insn() {
  let (sir, _) = execute_raw(
    r#"fun main() {
  imu n: int = 'Z' as int;
  showln(n);
}"#,
  );

  let has_cast = sir.iter().any(|i| matches!(i, Insn::Cast { .. }));

  assert!(has_cast, "expected Cast instruction in SIR");
}

#[test]
fn cast_int_to_float_emits_cast_insn() {
  let (sir, _) = execute_raw(
    r#"fun main() {
  imu f: float = 42 as float;
  showln(f);
}"#,
  );

  let cast = sir.iter().find(|i| matches!(i, Insn::Cast { .. }));

  assert!(cast.is_some(), "expected Cast instruction in SIR");

  if let Some(Insn::Cast { to_ty, .. }) = cast {
    // float type IDs are 15-17.
    assert!(
      to_ty.0 >= 15 && to_ty.0 <= 17,
      "expected to_ty in float range (15-17), got {}",
      to_ty.0
    );
  }
}

#[test]
fn cast_float_to_int_emits_cast_insn() {
  let (sir, _) = execute_raw(
    r#"fun main() {
  imu i: int = 3.14 as int;
  showln(i);
}"#,
  );

  let cast = sir.iter().find(|i| matches!(i, Insn::Cast { .. }));

  assert!(cast.is_some(), "expected Cast instruction in SIR");

  if let Some(Insn::Cast { to_ty, .. }) = cast {
    assert_eq!(to_ty.0, 8, "expected to_ty=8 (int), got {}", to_ty.0);
  }
}

#[test]
fn cast_in_showln_call() {
  // `showln(72 as char)` — Cast inside a call argument.
  assert_sir_structure(
    r#"fun main() {
  showln(72 as char);
}"#,
    |sir| {
      let has_cast = sir.iter().any(|i| matches!(i, Insn::Cast { .. }));

      assert!(has_cast, "expected Cast in showln argument");

      let has_call = sir.iter().any(|i| matches!(i, Insn::Call { .. }));

      assert!(has_call, "expected Call for showln");
    },
  );
}
