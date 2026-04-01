use crate::tests::common::{assert_annotations_stream, assert_sir_structure};

use zo_sir::{BinOp, Insn};
use zo_ty::{IntWidth, Ty, TyId};
use zo_value::ValueId;

// === BITWISE XOR ===

#[test]
fn test_bitxor_emits_binop() {
  let s32 = Ty::Int {
    signed: true,
    width: IntWidth::S32,
  };

  assert_annotations_stream(
    "0b1100 ^ 0b1010",
    &[
      (
        0,
        s32,
        Insn::ConstInt {
          dst: ValueId(0),
          value: 12,
          ty_id: TyId(8),
        },
      ),
      (
        1,
        s32,
        Insn::ConstInt {
          dst: ValueId(1),
          value: 10,
          ty_id: TyId(8),
        },
      ),
      (
        2,
        s32,
        // Constant folded: 0b1100 ^ 0b1010 = 6.
        Insn::ConstInt {
          dst: ValueId(2),
          value: 6,
          ty_id: TyId(8),
        },
      ),
    ],
  );
}

#[test]
fn test_bitxor_in_function_emits_eor() {
  assert_sir_structure(r#"fun xor(a: int, b: int) -> int { a ^ b }"#, |sir| {
    let has_xor = sir.iter().any(|i| {
      matches!(
        i,
        Insn::BinOp {
          op: BinOp::BitXor,
          ..
        }
      )
    });

    assert!(has_xor, "a ^ b should emit BinOp::BitXor, got: {sir:#?}");
  });
}

// === BITWISE AND ===

#[test]
fn test_bitand_in_function() {
  assert_sir_structure(r#"fun band(a: int, b: int) -> int { a & b }"#, |sir| {
    let has_and = sir.iter().any(|i| {
      matches!(
        i,
        Insn::BinOp {
          op: BinOp::BitAnd,
          ..
        }
      )
    });

    assert!(has_and, "a & b should emit BinOp::BitAnd, got: {sir:#?}");
  });
}

// === BITWISE OR ===

#[test]
fn test_bitor_in_function() {
  assert_sir_structure(r#"fun bor(a: int, b: int) -> int { a | b }"#, |sir| {
    let has_or = sir.iter().any(|i| {
      matches!(
        i,
        Insn::BinOp {
          op: BinOp::BitOr,
          ..
        }
      )
    });

    assert!(has_or, "a | b should emit BinOp::BitOr, got: {sir:#?}");
  });
}

// === COMPOUND BITWISE ASSIGNMENT ===

#[test]
fn test_xor_assign_emits_load_binop_store() {
  assert_sir_structure(
    r#"fun main() {
  mut x: int = 0;
  x ^= 3;
}"#,
    |sir| {
      // After `x ^= 3`, SIR should contain:
      //   Load(x) -> BinOp(BitXor, loaded, 3) -> Store(x).
      let has_load = sir.iter().any(|i| matches!(i, Insn::Load { .. }));
      let has_xor = sir.iter().any(|i| {
        matches!(
          i,
          Insn::BinOp {
            op: BinOp::BitXor,
            ..
          }
        )
      });
      let has_store = sir.iter().any(|i| matches!(i, Insn::Store { .. }));

      assert!(
        has_load && has_xor && has_store,
        "x ^= 3 should emit Load+BinOp(BitXor)+Store.\
         \n  load={has_load}, xor={has_xor}, store={has_store}\
         \n  sir: {sir:#?}"
      );
    },
  );
}

#[test]
fn test_and_assign_emits_load_binop_store() {
  assert_sir_structure(
    r#"fun main() {
  mut x: int = 0xff;
  x &= 0x0f;
}"#,
    |sir| {
      let has_and = sir.iter().any(|i| {
        matches!(
          i,
          Insn::BinOp {
            op: BinOp::BitAnd,
            ..
          }
        )
      });
      let has_store = sir.iter().any(|i| matches!(i, Insn::Store { .. }));

      assert!(
        has_and && has_store,
        "x &= 0x0f should emit BinOp(BitAnd)+Store.\
         \n  and={has_and}, store={has_store}\
         \n  sir: {sir:#?}"
      );
    },
  );
}

#[test]
fn test_or_assign_emits_load_binop_store() {
  assert_sir_structure(
    r#"fun main() {
  mut x: int = 0xf0;
  x |= 0x0f;
}"#,
    |sir| {
      let has_or = sir.iter().any(|i| {
        matches!(
          i,
          Insn::BinOp {
            op: BinOp::BitOr,
            ..
          }
        )
      });
      let has_store = sir.iter().any(|i| matches!(i, Insn::Store { .. }));

      assert!(
        has_or && has_store,
        "x |= 0x0f should emit BinOp(BitOr)+Store.\
         \n  or={has_or}, store={has_store}\
         \n  sir: {sir:#?}"
      );
    },
  );
}
