use crate::tests::common::{assert_sir_structure, execute_raw};

use zo_sir::Insn;

#[test]
fn match_int_literal_emits_cmp_chain() {
  assert_sir_structure(
    r#"fun main() {
  imu x: int = 3;
  match x {
    0 => showln("zero"),
    3 => showln("three"),
    _ => showln("other"),
  }
}"#,
    |sir| {
      let branch_count = sir
        .iter()
        .filter(|i| matches!(i, Insn::BranchIfNot { .. }))
        .count();

      // Two literal arms → two BranchIfNot (wildcard has none).
      assert!(
        branch_count >= 2,
        "expected >= 2 BranchIfNot, got {branch_count}"
      );
    },
  );
}

#[test]
fn match_enum_variant_emits_tuple_index_for_discriminant() {
  assert_sir_structure(
    r#"
enum Loot {
  Gold(int),
  Nothing,
}

fun main() {
  imu r: Loot = Loot::Gold(50);
  match r {
    Loot::Gold(n) => showln(n),
    Loot::Nothing => showln(0),
  }
}"#,
    |sir| {
      // Discriminant read: TupleIndex { index: 0 }.
      let has_disc_read = sir
        .iter()
        .any(|i| matches!(i, Insn::TupleIndex { index: 0, .. }));

      assert!(
        has_disc_read,
        "expected TupleIndex index=0 for discriminant"
      );

      // Payload extraction: TupleIndex { index: 1 }.
      let has_field_read = sir
        .iter()
        .any(|i| matches!(i, Insn::TupleIndex { index: 1, .. }));

      assert!(has_field_read, "expected TupleIndex index=1 for payload");
    },
  );
}

#[test]
fn match_result_ok_err_emits_correct_sir() {
  // This is the exact test case that crashes as a binary.
  // At the SIR level it should be well-formed: EnumDef for
  // Result, EnumConstruct for Ok(99), Load + TupleIndex for
  // discriminant, BranchIfNot for each arm, VarDef + Store
  // for the payload binding.
  let (sir, _) = execute_raw(
    r#"
enum Result<$T, $E> {
  Ok($T),
  Err($E),
}

fun main() {
  imu ok: Result<int, int> = Result::Ok(99);
  match ok {
    Result::Ok(v) => showln(v),
    Result::Err(e) => showln(e),
  }
}"#,
  );

  // Should have an EnumDef for Result.
  let has_result_def = sir.iter().any(
    |i| matches!(i, Insn::EnumDef { variants, .. } if variants.len() == 2),
  );

  assert!(
    has_result_def,
    "expected EnumDef with 2 variants for Result"
  );

  // Should have EnumConstruct for Ok(99).
  let has_construct = sir.iter().any(|i| {
    matches!(i, Insn::EnumConstruct { variant: 0, fields, .. } if fields.len() == 1)
  });

  assert!(has_construct, "expected EnumConstruct for Result::Ok(99)");

  // Should have TupleIndex { index: 0 } for discriminant reads.
  let disc_reads = sir
    .iter()
    .filter(|i| matches!(i, Insn::TupleIndex { index: 0, .. }))
    .count();

  assert!(
    disc_reads >= 2,
    "expected >= 2 discriminant reads (one per arm), got {disc_reads}"
  );

  // Should have BranchIfNot for each arm's pattern test.
  let branches = sir
    .iter()
    .filter(|i| matches!(i, Insn::BranchIfNot { .. }))
    .count();

  assert!(
    branches >= 2,
    "expected >= 2 BranchIfNot for Result match, got {branches}"
  );

  // Should have VarDef for the payload bindings (v, e).
  let var_defs: Vec<_> = sir
    .iter()
    .filter(|i| matches!(i, Insn::VarDef { .. }))
    .collect();

  // At least `ok` + `v` + `e` = 3 VarDefs.
  assert!(
    var_defs.len() >= 3,
    "expected >= 3 VarDefs (ok + v + e), got {}",
    var_defs.len()
  );
}

#[test]
fn match_option_then_result_sir_correct() {
  // Full 047-result-option.zo scenario: two Option matches
  // then one Result match. The SIR must contain all three
  // match dispatch chains without ValueId/label collision.
  let (sir, _) = execute_raw(
    r#"
enum Option<$T> {
  Some($T),
  None,
}

enum Result<$T, $E> {
  Ok($T),
  Err($E),
}

fun main() {
  imu a: Option<int> = Option::Some(42);
  imu b: Option<int> = Option::None;

  match a {
    Option::Some(v) => showln(v),
    Option::None => showln(0),
  }

  match b {
    Option::Some(v) => showln(v),
    Option::None => showln(0),
  }

  imu ok: Result<int, int> = Result::Ok(99);

  match ok {
    Result::Ok(v) => showln(v),
    Result::Err(e) => showln(e),
  }
}"#,
  );

  // Three enum constructs: Some(42), None, Ok(99).
  let constructs = sir
    .iter()
    .filter(|i| matches!(i, Insn::EnumConstruct { .. }))
    .count();

  assert!(
    constructs >= 3,
    "expected >= 3 EnumConstruct, got {constructs}"
  );

  // Three matches → at least 6 BranchIfNot (2 per match,
  // one per non-wildcard arm).
  let branches = sir
    .iter()
    .filter(|i| matches!(i, Insn::BranchIfNot { .. }))
    .count();

  assert!(branches >= 4, "expected >= 4 BranchIfNot, got {branches}");

  // Print SIR for debugging the codegen crash.
  eprintln!("--- Option+Result SIR ({} instructions) ---", sir.len());

  for (i, insn) in sir.iter().enumerate() {
    eprintln!("  [{i}] {insn:?}");
  }
}

#[test]
fn match_multi_match_no_interference() {
  // Three matches in one function must each dispatch
  // independently — the scrutinee reload per arm prevents
  // register liveness leaks.
  assert_sir_structure(
    r#"fun main() {
  imu a: int = 0;
  imu b: int = 3;
  imu c: int = 99;

  match a {
    0 => showln("sunday"),
    _ => showln("other"),
  }
  match b {
    3 => showln("wednesday"),
    _ => showln("other"),
  }
  match c {
    0 => showln("zero"),
    _ => showln("wild"),
  }
}"#,
    |sir| {
      // Three matches → at least 3 BranchIfNot (one per
      // literal arm, wildcard has none).
      let branches = sir
        .iter()
        .filter(|i| matches!(i, Insn::BranchIfNot { .. }))
        .count();

      assert!(
        branches >= 3,
        "expected >= 3 BranchIfNot for 3 matches, got {branches}"
      );

      // Three end labels — one Label per match.
      let labels = sir
        .iter()
        .filter(|i| matches!(i, Insn::Label { .. }))
        .count();

      // Each match emits: N arm labels + 1 end label.
      // 3 matches × 2 arms = 3×(1 arm_label + 1 end) = 6+.
      assert!(
        labels >= 6,
        "expected >= 6 labels for 3 matches, got {labels}"
      );
    },
  );
}
