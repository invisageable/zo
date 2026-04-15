use crate::tests::common::{assert_sir_structure, execute_raw};

use zo_sir::Insn;

#[test]
fn test_array_literal_produces_sir() {
  assert_sir_structure(
    r#"fun main() {
  imu x: []int = [1, 2, 3];
}"#,
    |sir| {
      let has_array =
        sir.iter().any(|i| matches!(i, Insn::ArrayLiteral { .. }));

      assert!(has_array, "expected ArrayLiteral in SIR");
    },
  );
}

#[test]
fn test_array_index_produces_sir() {
  assert_sir_structure(
    r#"fun main() {
  imu x: []int = [10, 20, 30];
  imu v: int = x[0];
}"#,
    |sir| {
      let has_index = sir.iter().any(|i| matches!(i, Insn::ArrayIndex { .. }));

      assert!(has_index, "expected ArrayIndex in SIR");
    },
  );
}

#[test]
fn test_array_with_showln() {
  assert_sir_structure(
    r#"fun main() {
  imu x: []int = [10, 25, 50];
  imu v: int = x[0];
  showln(v);
}"#,
    |sir| {
      let calls = sir
        .iter()
        .filter(|i| matches!(i, Insn::Call { .. }))
        .count();

      assert!(
        calls >= 1,
        "expected at least 1 Call for showln, got {}",
        calls
      );
    },
  );
}

#[test]
fn test_array_binop_two_indices() {
  // a[0] + a[1] should produce two ArrayIndex then BinOp.
  assert_sir_structure(
    r#"fun main() {
  imu a: []int = [5, 12, 8];
  imu c: int = a[0] + a[1];
  showln(c);
}"#,
    |sir| {
      let arr_idx_count = sir
        .iter()
        .filter(|i| matches!(i, Insn::ArrayIndex { .. }))
        .count();

      assert_eq!(
        arr_idx_count, 2,
        "expected 2 ArrayIndex, got {}",
        arr_idx_count
      );

      // BinOp should reference both ArrayIndex results.
      let has_binop = sir.iter().any(|i| {
        matches!(
          i,
          Insn::BinOp {
            op: zo_sir::BinOp::Add,
            ..
          }
        )
      });

      assert!(has_binop, "expected Add BinOp");
    },
  );
}

#[test]
fn test_array_with_interp_showln() {
  // Array + interpolation with prefix text.
  assert_sir_structure(
    r#"fun main() {
  imu x: []int = [10, 25, 50];
  imu v: int = x[0];
  showln("value: {v}");
}"#,
    |sir| {
      let calls = sir
        .iter()
        .filter(|i| matches!(i, Insn::Call { .. }))
        .count();

      assert!(
        calls >= 2,
        "expected >= 2 Call instructions for \
         interpolation desugaring, got {}",
        calls
      );
    },
  );
}

#[test]
fn test_array_store_produces_sir() {
  assert_sir_structure(
    r#"fun main() {
  mut arr: []int = [0, 0, 0];
  arr[0] = 10;
  showln(arr[0]);
}"#,
    |sir| {
      let has_store = sir.iter().any(|i| matches!(i, Insn::ArrayStore { .. }));

      assert!(has_store, "expected ArrayStore instruction");
    },
  );
}

#[test]
fn test_array_store_in_loop() {
  assert_sir_structure(
    r#"fun main() {
  mut arr: []int = [0, 0, 0];
  mut i: int = 0;
  while i < 3 {
    arr[i] = i * i;
    i = i + 1;
  }
  showln(arr[2]);
}"#,
    |sir| {
      let store_count = sir
        .iter()
        .filter(|i| matches!(i, Insn::ArrayStore { .. }))
        .count();

      assert!(
        store_count >= 1,
        "expected ArrayStore in loop, got {store_count}"
      );
    },
  );
}

#[test]
fn test_array_store_read_modify_write() {
  assert_sir_structure(
    r#"fun main() {
  mut arr: []int = [100, 80, 60];
  arr[0] = arr[0] - 25;
  showln(arr[0]);
}"#,
    |sir| {
      // Should have both ArrayIndex (read) and ArrayStore (write).
      let has_index = sir.iter().any(|i| matches!(i, Insn::ArrayIndex { .. }));

      let has_store = sir.iter().any(|i| matches!(i, Insn::ArrayStore { .. }));

      assert!(has_index, "expected ArrayIndex for read");
      assert!(has_store, "expected ArrayStore for write");
    },
  );
}

#[test]
fn test_array_push_produces_sir() {
  let (insns, _) = execute_raw(
    r#"fun main() {
  mut arr: []int = [];
  arr.push(10);
}"#,
  );

  let has_push = insns.iter().any(|i| matches!(i, Insn::ArrayPush { .. }));

  assert!(has_push, "expected ArrayPush in SIR");
}

#[test]
fn test_array_len_produces_sir() {
  let (insns, _) = execute_raw(
    r#"fun main() {
  imu arr: []int = [10, 20, 30];
  imu n: int = arr.len;
  showln(n);
}"#,
  );

  let has_len = insns.iter().any(|i| matches!(i, Insn::ArrayLen { .. }));

  assert!(has_len, "expected ArrayLen in SIR");

  // ArrayLen result type should be int (TyId 8 = s32).
  if let Some(Insn::ArrayLen { ty_id, .. }) =
    insns.iter().find(|i| matches!(i, Insn::ArrayLen { .. }))
  {
    assert_eq!(
      ty_id.0, 8,
      "expected ArrayLen ty_id=8 (int), got {}",
      ty_id.0
    );
  }
}

#[test]
fn test_string_index_produces_char_type() {
  let (insns, _) = execute_raw(
    r#"fun main() {
  imu s: str = "hello";
  showln(s[0]);
}"#,
  );

  // Find the ArrayIndex instruction.
  let arr_idx = insns.iter().find(|i| matches!(i, Insn::ArrayIndex { .. }));

  assert!(arr_idx.is_some(), "expected ArrayIndex in SIR");

  if let Some(Insn::ArrayIndex { ty_id, .. }) = arr_idx {
    // Char type ID is 3 (TyChecker::new() registration).
    assert_eq!(
      ty_id.0, 3,
      "expected ArrayIndex ty_id=3 (char), got {}",
      ty_id.0
    );
  }
}

#[test]
fn test_string_len_produces_array_len() {
  let (insns, _) = execute_raw(
    r#"fun main() {
  imu s: str = "hello";
  imu n: int = s.len;
  showln(n);
}"#,
  );

  let has_len = insns.iter().any(|i| matches!(i, Insn::ArrayLen { .. }));

  assert!(has_len, "expected ArrayLen for str.len");

  // ArrayLen result type should be int (TyId 8).
  if let Some(Insn::ArrayLen { ty_id, .. }) =
    insns.iter().find(|i| matches!(i, Insn::ArrayLen { .. }))
  {
    assert_eq!(
      ty_id.0, 8,
      "expected str.len ty_id=8 (int), got {}",
      ty_id.0
    );
  }
}

#[test]
fn test_interp_with_prefix_no_array() {
  // Interpolation with prefix text: showln("value: {x}")
  // desugars to show("value: ") + showln(x).
  assert_sir_structure(
    r#"fun main() {
  imu x: int = 42;
  showln("value: {x}");
  showln("done");
}"#,
    |sir| {
      let calls = sir
        .iter()
        .filter(|i| matches!(i, Insn::Call { .. }))
        .count();

      // show("value: ") + showln(x) + showln("done")
      assert!(calls >= 3, "expected >= 3 Call instructions, got {}", calls);
    },
  );
}
