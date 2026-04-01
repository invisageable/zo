use crate::tests::common::execute_raw;

use zo_sir::Insn;

/// Folded operands must become `Nop` — instruction indices
/// stay stable while dead code is eliminated in-place.

#[test]
fn test_fold_int_nops_operands() {
  let (insns, annotations) = execute_raw("1 + 2");

  // 3 instructions total: Nop, Nop, ConstInt(3).
  assert_eq!(insns.len(), 3, "insn count: {insns:#?}");
  assert!(matches!(insns[0], Insn::Nop), "insns[0]: {:#?}", insns[0]);
  assert!(matches!(insns[1], Insn::Nop), "insns[1]: {:#?}", insns[1]);
  assert!(
    matches!(insns[2], Insn::ConstInt { value: 3, .. }),
    "insns[2]: {:#?}",
    insns[2]
  );

  // Only 1 annotation — the folded result.
  assert_eq!(annotations.len(), 1, "annotation count: {annotations:#?}");
}

#[test]
fn test_fold_bool_nops_operands() {
  let (insns, annotations) = execute_raw("1 == 1");

  let nop_count = insns.iter().filter(|i| matches!(i, Insn::Nop)).count();
  let has_bool = insns
    .iter()
    .any(|i| matches!(i, Insn::ConstBool { value: true, .. }));

  assert_eq!(nop_count, 2, "expected 2 Nops: {insns:#?}");
  assert!(has_bool, "expected ConstBool(true): {insns:#?}");
  assert_eq!(annotations.len(), 1, "annotations: {annotations:#?}");
}

#[test]
fn test_fold_chain_nops_all_intermediates() {
  // 2 + 3 * 4 = 14.
  // First fold: 3 * 4 = 12 → nops 3 and 4.
  // Second fold: 2 + 12 = 14 → nops 2 and 12.
  let (insns, _) = execute_raw("fun main() { imu x: int = 2 + 3 * 4; }");

  let nop_count = insns.iter().filter(|i| matches!(i, Insn::Nop)).count();
  let has_14 = insns
    .iter()
    .any(|i| matches!(i, Insn::ConstInt { value: 14, .. }));

  assert!(nop_count >= 4, "expected >= 4 Nops: {insns:#?}");
  assert!(has_14, "expected ConstInt(14): {insns:#?}");
}

#[test]
fn test_fold_xor_nops_operands() {
  let (insns, _) = execute_raw("0b1100 ^ 0b1010");

  let nop_count = insns.iter().filter(|i| matches!(i, Insn::Nop)).count();
  let has_6 = insns
    .iter()
    .any(|i| matches!(i, Insn::ConstInt { value: 6, .. }));

  assert_eq!(nop_count, 2, "expected 2 Nops: {insns:#?}");
  assert!(has_6, "expected ConstInt(6): {insns:#?}");
}

#[test]
fn test_fold_preserves_instruction_count() {
  // After folding, instruction count must stay the same
  // (Nops replace operands, nothing is removed).
  let (insns, _) = execute_raw("10 - 3");

  // 3 instructions: Nop(10), Nop(3), ConstInt(7).
  assert_eq!(insns.len(), 3);
}

#[test]
fn test_no_fold_for_runtime_binop() {
  // Runtime values can't fold — no Nops.
  let (insns, _) = execute_raw("fun add(a: int, b: int) -> int { a + b }");

  let nop_count = insns.iter().filter(|i| matches!(i, Insn::Nop)).count();
  let has_binop = insns.iter().any(|i| matches!(i, Insn::BinOp { .. }));

  assert_eq!(nop_count, 0, "no Nops for runtime: {insns:#?}");
  assert!(has_binop, "expected BinOp: {insns:#?}");
}
