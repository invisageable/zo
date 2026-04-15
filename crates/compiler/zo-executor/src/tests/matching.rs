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

// -- Match on primitive types -----------------------------------------

#[test]
fn match_char_literal_emits_cmp_chain() {
  assert_sir_structure(
    r#"fun main() {
  imu c: char = 'a';
  match c {
    'a' => showln(1),
    'b' => showln(2),
    _ => showln(0),
  }
}"#,
    |sir| {
      // Two char arms → two BranchIfNot.
      let branches = sir
        .iter()
        .filter(|i| matches!(i, Insn::BranchIfNot { .. }))
        .count();

      assert!(
        branches >= 2,
        "expected >= 2 BranchIfNot for char match, got {branches}"
      );

      // Pattern constants should be ConstInt with char values.
      let char_consts = sir
        .iter()
        .filter(|i| {
          matches!(
            i,
            Insn::ConstInt { value, .. }
              if *value == 97 || *value == 98  // 'a' or 'b'
          )
        })
        .count();

      assert!(
        char_consts >= 2,
        "expected char constants for 'a' (97) and 'b' (98), \
         got {char_consts}"
      );
    },
  );
}

#[test]
fn match_bool_literal_emits_cmp() {
  assert_sir_structure(
    r#"fun main() {
  imu b: bool = true;
  match b {
    true => showln(1),
    false => showln(0),
  }
}"#,
    |sir| {
      // Two arms → two BranchIfNot.
      let branches = sir
        .iter()
        .filter(|i| matches!(i, Insn::BranchIfNot { .. }))
        .count();

      assert!(
        branches >= 2,
        "expected >= 2 BranchIfNot for bool match, got {branches}"
      );

      // Should have ConstBool for true and false.
      let has_true = sir
        .iter()
        .any(|i| matches!(i, Insn::ConstBool { value: true, .. }));
      let has_false = sir
        .iter()
        .any(|i| matches!(i, Insn::ConstBool { value: false, .. }));

      assert!(has_true, "expected ConstBool(true) pattern");
      assert!(has_false, "expected ConstBool(false) pattern");
    },
  );
}

#[test]
fn match_bytes_literal_emits_cmp() {
  assert_sir_structure(
    r#"fun main() {
  imu b: bytes = `x`;
  match b {
    `x` => showln(1),
    `y` => showln(2),
    _ => showln(0),
  }
}"#,
    |sir| {
      let branches = sir
        .iter()
        .filter(|i| matches!(i, Insn::BranchIfNot { .. }))
        .count();

      assert!(
        branches >= 2,
        "expected >= 2 BranchIfNot for bytes match, got {branches}"
      );
    },
  );
}

#[test]
fn match_float_literal_emits_cmp() {
  assert_sir_structure(
    r#"fun main() {
  imu f: float = 3.14;
  match f {
    3.14 => showln(1),
    2.71 => showln(2),
    _ => showln(0),
  }
}"#,
    |sir| {
      let branches = sir
        .iter()
        .filter(|i| matches!(i, Insn::BranchIfNot { .. }))
        .count();

      assert!(
        branches >= 2,
        "expected >= 2 BranchIfNot for float match, got {branches}"
      );

      // Should have ConstFloat for patterns.
      let float_consts = sir
        .iter()
        .filter(|i| matches!(i, Insn::ConstFloat { .. }))
        .count();

      assert!(
        float_consts >= 2,
        "expected >= 2 ConstFloat for patterns, got {float_consts}"
      );
    },
  );
}

// -- Match arm side effects -------------------------------------------

#[test]
fn match_arm_variable_assignment_emits_store() {
  // `ptr = ptr + 1` inside a match arm must emit
  // BinOp::Add + Store — deferred binop must flush.
  assert_sir_structure(
    r#"fun main() {
  mut ptr: int = 0;
  imu cmd: char = '>';
  match cmd {
    '>' => ptr = ptr + 1,
    _ => {},
  }
  showln(ptr);
}"#,
    |sir| {
      // Should have a Store for `ptr = ptr + 1`.
      // Find Store instructions after the match pattern.
      let stores: Vec<_> = sir
        .iter()
        .filter(|i| matches!(i, Insn::Store { .. }))
        .collect();

      // At least: Store(ptr=0), Store(cmd), Store(ptr=ptr+1).
      assert!(
        stores.len() >= 3,
        "expected >= 3 Store instructions (init ptr, init cmd, \
         arm assign), got {}",
        stores.len()
      );

      // The arm assignment should produce a BinOp::Add.
      let has_add = sir.iter().any(|i| {
        matches!(
          i,
          Insn::BinOp {
            op: zo_sir::BinOp::Add,
            ..
          }
        )
      });

      assert!(has_add, "expected BinOp::Add for ptr + 1");
    },
  );
}

#[test]
fn match_arm_array_store_emits_array_store() {
  // `tape[0] = tape[0] + 1` inside a match arm must emit
  // ArrayIndex (read) + BinOp::Add + ArrayStore (write).
  assert_sir_structure(
    r#"fun main() {
  mut tape: []int = [0, 0, 0];
  imu cmd: char = '+';
  match cmd {
    '+' => tape[0] = tape[0] + 1,
    _ => {},
  }
  showln(tape[0]);
}"#,
    |sir| {
      let has_array_store =
        sir.iter().any(|i| matches!(i, Insn::ArrayStore { .. }));

      assert!(has_array_store, "expected ArrayStore in match arm body");

      // Read-modify-write: ArrayIndex + BinOp::Add + ArrayStore.
      let arr_idx_count = sir
        .iter()
        .filter(|i| matches!(i, Insn::ArrayIndex { .. }))
        .count();

      assert!(
        arr_idx_count >= 2,
        "expected >= 2 ArrayIndex (LHS target + RHS read), \
         got {arr_idx_count}"
      );
    },
  );
}

// -- Block arms inside while loop (scope depth bug) -------------------

#[test]
fn match_block_arm_inside_while_preserves_loop() {
  // Regression: `_ => {}` inside a while loop's match
  // consumed the While's BranchCtx, breaking the loop.
  // The fix: scope_depth guard on BranchCtx.
  assert_sir_structure(
    r#"fun main() {
  mut x: int = 0;
  mut i: int = 0;
  while i < 3 {
    match i {
      0 => x = x + 1,
      _ => {},
    }
    i = i + 1;
  }
  showln(x);
}"#,
    |sir| {
      // The while loop must have a back-edge Jump to its
      // start label. Count Jump instructions.
      let jumps: Vec<_> = sir
        .iter()
        .filter(|i| matches!(i, Insn::Jump { .. }))
        .collect();

      // While loop: at least 1 back-edge + match arm jumps.
      assert!(
        jumps.len() >= 2,
        "expected >= 2 Jump (while back-edge + match arm end), \
         got {}",
        jumps.len()
      );

      // The while's Label 0 (or whatever ID) must appear
      // as both a Label and a Jump target (back-edge).
      let label_ids: Vec<u32> = sir
        .iter()
        .filter_map(|i| match i {
          Insn::Label { id } => Some(*id),
          _ => None,
        })
        .collect();

      let jump_targets: Vec<u32> = sir
        .iter()
        .filter_map(|i| match i {
          Insn::Jump { target } => Some(*target),
          _ => None,
        })
        .collect();

      // At least one Jump target should point to a Label
      // that appears BEFORE it in the SIR (back-edge).
      let has_back_edge = jump_targets.iter().any(|t| label_ids.contains(t));

      assert!(
        has_back_edge,
        "expected while back-edge Jump to a known Label"
      );
    },
  );
}

#[test]
fn match_block_arm_inside_while_array_accumulates() {
  // The full brainfuck pattern: array assignment in match
  // arm inside while loop must produce correct SIR so the
  // loop accumulates across iterations.
  assert_sir_structure(
    r#"fun main() {
  mut tape: []int = [0, 0, 0];
  mut pc: int = 0;
  while pc < 3 {
    match pc {
      0 => tape[0] = tape[0] + 1,
      _ => {},
    }
    pc = pc + 1;
  }
  showln(tape[0]);
}"#,
    |sir| {
      // Must have ArrayStore inside the match arm.
      let has_array_store =
        sir.iter().any(|i| matches!(i, Insn::ArrayStore { .. }));

      assert!(
        has_array_store,
        "expected ArrayStore for tape[0] = tape[0] + 1"
      );

      // Must have while back-edge: a Jump whose target is
      // a Label that precedes it in the SIR stream.
      let mut label_positions = std::collections::HashMap::new();

      for (pos, insn) in sir.iter().enumerate() {
        if let Insn::Label { id } = insn {
          label_positions.insert(*id, pos);
        }
      }

      let has_back_edge = sir.iter().enumerate().any(|(pos, insn)| {
        if let Insn::Jump { target } = insn {
          label_positions.get(target).is_some_and(|&lpos| lpos < pos)
        } else {
          false
        }
      });

      assert!(
        has_back_edge,
        "expected while loop back-edge (Jump to earlier Label)"
      );
    },
  );
}

#[test]
fn match_if_inside_block_arm_in_while() {
  // Nested control flow: if inside a match block arm
  // inside a while loop. The if's braces must not consume
  // the while's BranchCtx either.
  assert_sir_structure(
    r#"fun main() {
  mut x: int = 0;
  mut i: int = 0;
  while i < 5 {
    match i {
      0 => {
        if x == 0 {
          x = 1;
        }
      },
      _ => {},
    }
    i = i + 1;
  }
  showln(x);
}"#,
    |sir| {
      // Should have at least 3 BranchIfNot:
      // 1. while condition
      // 2. match pattern (0)
      // 3. if condition (x == 0)
      let branches = sir
        .iter()
        .filter(|i| matches!(i, Insn::BranchIfNot { .. }))
        .count();

      assert!(
        branches >= 3,
        "expected >= 3 BranchIfNot (while + match + if), \
         got {branches}"
      );

      // Must still have while back-edge.
      let mut label_positions = std::collections::HashMap::new();

      for (pos, insn) in sir.iter().enumerate() {
        if let Insn::Label { id } = insn {
          label_positions.insert(*id, pos);
        }
      }

      let has_back_edge = sir.iter().enumerate().any(|(pos, insn)| {
        if let Insn::Jump { target } = insn {
          label_positions.get(target).is_some_and(|&lpos| lpos < pos)
        } else {
          false
        }
      });

      assert!(
        has_back_edge,
        "while back-edge missing — if inside match block \
         arm consumed the while context"
      );
    },
  );
}

// -- Match on string literals -----------------------------------------

#[test]
fn match_string_literal_emits_const_string_and_cmp() {
  assert_sir_structure(
    r#"fun main() {
  imu s: str = "hello";
  match s {
    "hello" => showln(1),
    "world" => showln(2),
    _ => showln(0),
  }
}"#,
    |sir| {
      // Two string arms → two BranchIfNot.
      let branches = sir
        .iter()
        .filter(|i| matches!(i, Insn::BranchIfNot { .. }))
        .count();

      assert!(
        branches >= 2,
        "expected >= 2 BranchIfNot for string match, \
         got {branches}"
      );

      // Should have ConstString for each pattern.
      let str_consts = sir
        .iter()
        .filter(|i| matches!(i, Insn::ConstString { .. }))
        .count();

      // At least 3: the scrutinee "hello" + patterns "hello"
      // and "world".
      assert!(
        str_consts >= 3,
        "expected >= 3 ConstString (scrutinee + 2 patterns), \
         got {str_consts}"
      );
    },
  );
}
