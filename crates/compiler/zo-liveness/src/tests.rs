//! ```sh
//! cargo test -p zo-liveness --lib
//! ```

use crate::insn::{compute_value_ids, insn_uses};

use zo_sir::Insn;
use zo_ty::TyId;
use zo_value::ValueId;

// ===== insn_uses =====

#[test]
fn array_store_uses_three_values() {
  let insn = Insn::ArrayStore {
    array: ValueId(0),
    index: ValueId(1),
    value: ValueId(2),
    ty_id: TyId(8),
  };

  let uses = insn_uses(&insn);

  assert_eq!(uses.len(), 3);
  assert_eq!(uses[0], ValueId(0)); // array
  assert_eq!(uses[1], ValueId(1)); // index
  assert_eq!(uses[2], ValueId(2)); // value
}

#[test]
fn array_index_uses_two_values() {
  let insn = Insn::ArrayIndex {
    dst: ValueId(3),
    array: ValueId(0),
    index: ValueId(1),
    ty_id: TyId(8),
  };

  let uses = insn_uses(&insn);

  assert_eq!(uses.len(), 2);
  assert_eq!(uses[0], ValueId(0));
  assert_eq!(uses[1], ValueId(1));
}

#[test]
fn field_store_uses_two_values() {
  let insn = Insn::FieldStore {
    base: ValueId(0),
    index: 1,
    value: ValueId(2),
    ty_id: TyId(8),
  };

  let uses = insn_uses(&insn);

  assert_eq!(uses.len(), 2);
  assert_eq!(uses[0], ValueId(0));
  assert_eq!(uses[1], ValueId(2));
}

// ===== compute_value_ids =====

#[test]
fn array_store_produces_no_value() {
  let insns = vec![Insn::ArrayStore {
    array: ValueId(0),
    index: ValueId(1),
    value: ValueId(2),
    ty_id: TyId(8),
  }];

  let ids = compute_value_ids(&insns);

  assert_eq!(ids.len(), 1);
  assert!(ids[0].is_none());
}

#[test]
fn array_index_produces_value() {
  let insns = vec![Insn::ArrayIndex {
    dst: ValueId(5),
    array: ValueId(0),
    index: ValueId(1),
    ty_id: TyId(8),
  }];

  let ids = compute_value_ids(&insns);

  assert_eq!(ids.len(), 1);
  assert_eq!(ids[0], Some(ValueId(5)));
}
