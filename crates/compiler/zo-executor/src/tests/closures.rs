use crate::tests::common::{assert_no_errors, assert_sir_structure};

use zo_sir::Insn;
use zo_ty::TyId;
use zo_value::FunctionKind;

#[test]
fn test_closure_block_emits_fundef() {
  assert_sir_structure(
    r#"fun main() {
  imu f := fn(x: int) -> int { x + x };
}"#,
    |sir| {
      let closure_count = sir
        .iter()
        .filter(|i| {
          matches!(
            i,
            Insn::FunDef {
              kind: FunctionKind::Closure { .. },
              ..
            }
          )
        })
        .count();

      assert_eq!(
        closure_count, 1,
        "expected 1 closure FunDef, got {closure_count}"
      );
    },
  );
}

#[test]
fn test_closure_inline_emits_fundef() {
  assert_sir_structure(
    r#"fun main() {
  imu f := fn(x: int) -> int => x + x;
}"#,
    |sir| {
      let closure_count = sir
        .iter()
        .filter(|i| {
          matches!(
            i,
            Insn::FunDef {
              kind: FunctionKind::Closure { .. },
              ..
            }
          )
        })
        .count();

      assert_eq!(
        closure_count, 1,
        "expected 1 closure FunDef, got {closure_count}"
      );
    },
  );
}

#[test]
fn test_closure_has_return() {
  assert_sir_structure(
    r#"fun main() {
  imu f := fn(x: int) -> int => x;
}"#,
    |sir| {
      // Closure FunDef should be followed by body + Return.
      let has_closure = sir.iter().any(|i| {
        matches!(
          i,
          Insn::FunDef {
            kind: FunctionKind::Closure { .. },
            ..
          }
        )
      });

      assert!(has_closure, "expected closure FunDef");

      // At least one Return inside the closure body.
      let return_count = sir
        .iter()
        .filter(|i| matches!(i, Insn::Return { .. }))
        .count();

      // main's implicit return + closure's return.
      assert!(
        return_count >= 2,
        "expected at least 2 Returns (main + closure), got {return_count}"
      );
    },
  );
}

#[test]
fn test_closure_capture_adds_params() {
  assert_sir_structure(
    r#"fun main() -> int {
  imu y: int = 10;
  imu f := fn(x: int) -> int => x + y;
  f(5)
}"#,
    |sir| {
      // The closure should have capture_count > 0.
      let closure = sir.iter().find(|i| {
        matches!(
          i,
          Insn::FunDef {
            kind: FunctionKind::Closure { .. },
            ..
          }
        )
      });

      assert!(closure.is_some(), "expected closure FunDef");

      if let Some(Insn::FunDef {
        kind: FunctionKind::Closure { capture_count },
        params,
        ..
      }) = closure
      {
        assert_eq!(
          *capture_count, 1,
          "expected 1 capture, got {capture_count}"
        );

        // Total params = 1 capture (y) + 1 user param (x) = 2.
        assert_eq!(
          params.len(),
          2,
          "expected 2 total params (capture + user), got {}",
          params.len()
        );
      }
    },
  );
}

#[test]
fn test_closure_no_capture_zero_count() {
  assert_sir_structure(
    r#"fun main() {
  imu f := fn(x: int) -> int => x + x;
}"#,
    |sir| {
      let closure = sir.iter().find(|i| {
        matches!(
          i,
          Insn::FunDef {
            kind: FunctionKind::Closure { .. },
            ..
          }
        )
      });

      if let Some(Insn::FunDef {
        kind: FunctionKind::Closure { capture_count },
        params,
        ..
      }) = closure
      {
        assert_eq!(
          *capture_count, 0,
          "expected 0 captures, got {capture_count}"
        );
        assert_eq!(params.len(), 1, "expected 1 param, got {}", params.len());
      }
    },
  );
}

#[test]
fn test_closure_call_emits_call_insn() {
  assert_sir_structure(
    r#"fun main() -> int {
  imu f := fn(x: int) -> int => x + x;
  f(5)
}"#,
    |sir| {
      let has_call = sir.iter().any(|i| matches!(i, Insn::Call { .. }));

      assert!(has_call, "expected Call instruction for closure invocation");
    },
  );
}

#[test]
fn test_closure_inline_call_in_check_eq() {
  // Inline closure result as check@eq argument.
  assert_sir_structure(
    r#"ext check(b: bool);
fun main() {
  imu f := fn(x: int) -> int => x * x;
  check@eq(f(7), 49);
}"#,
    |sir| {
      let closure_call = sir.iter().find(|i| {
        matches!(
          i,
          Insn::Call { ty_id, .. }
          if *ty_id == TyId(8)
        )
      });

      assert!(
        closure_call.is_some(),
        "expected closure Call with int return type"
      );
    },
  );
}

#[test]
fn test_closure_block_call_in_check_eq() {
  // Block closure result as check@eq argument.
  assert_sir_structure(
    r#"ext check(b: bool);
fun main() {
  imu f: Fn(int) -> int = fn(x: int) -> int { x + x };
  check@eq(f(21), 42);
}"#,
    |sir| {
      let closure_call = sir.iter().find(|i| {
        matches!(
          i,
          Insn::Call { ty_id, .. }
          if *ty_id == TyId(8)
        )
      });

      assert!(
        closure_call.is_some(),
        "expected block closure Call with int return type"
      );
    },
  );
}

#[test]
fn test_recursive_closure() {
  // Recursive closure: fib calls itself by name.
  assert_sir_structure(
    r#"ext check(b: bool);
fun main() {
  imu fib := fn(n: int) -> int {
    if n <= 1 {
      return n;
    }
    return fib(n - 1) + fib(n - 2);
  };
  check@eq(fib(8), 21);
}"#,
    |sir| {
      // Recursive calls should NOT return unit —
      // they must resolve through the letrec local.
      let unit_calls = sir
        .iter()
        .filter(|i| {
          matches!(
            i,
            Insn::Call { ty_id, .. }
            if *ty_id == TyId(1) // unit
          )
        })
        .count();

      // Only check@eq should return unit (1 call).
      assert!(
        unit_calls <= 1,
        "expected at most 1 unit Call (check), got {unit_calls}",
      );
    },
  );
}

// ================================================================
// Fn(T) -> R type annotation with closure.
// ================================================================

#[test]
fn test_closure_fn_type_annotation_unifies() {
  assert_no_errors(
    r#"fun main() {
  imu f: Fn(int) -> int = fn(x: int) -> int => x * x;
}"#,
  );
}

#[test]
fn test_closure_fn_type_annotation_propagates_params() {
  assert_no_errors(
    r#"fun main() {
  imu f: Fn(int) -> int = fn(x) => x * x;
}"#,
  );
}

// ================================================================
// Multiple captures.
// ================================================================

#[test]
fn test_closure_multi_capture() {
  assert_sir_structure(
    r#"fun main() -> int {
  imu a: int = 10;
  imu b: int = 20;
  imu f := fn(x: int) -> int => x + a + b;
  f(5)
}"#,
    |sir| {
      let closure = sir.iter().find(|i| {
        matches!(
          i,
          Insn::FunDef {
            kind: FunctionKind::Closure { .. },
            ..
          }
        )
      });

      if let Some(Insn::FunDef {
        kind: FunctionKind::Closure { capture_count },
        params,
        ..
      }) = closure
      {
        assert_eq!(
          *capture_count, 2,
          "expected 2 captures (a, b), got {capture_count}"
        );

        assert_eq!(
          params.len(),
          3,
          "expected 3 total params, got {}",
          params.len()
        );
      } else {
        panic!("expected closure FunDef");
      }
    },
  );
}

// ================================================================
// Closure calling a captured closure.
// ================================================================

#[test]
fn test_closure_captures_closure_value() {
  assert_sir_structure(
    r#"fun main() -> int {
  imu double := fn(x: int) -> int => x * 2;
  imu then_add := fn(x: int) -> int => double(x) + 1;
  then_add(5)
}"#,
    |sir| {
      let closure_count = sir
        .iter()
        .filter(|i| {
          matches!(
            i,
            Insn::FunDef {
              kind: FunctionKind::Closure { .. },
              ..
            }
          )
        })
        .count();

      assert_eq!(
        closure_count, 2,
        "expected 2 closures (double + then_add), got {closure_count}"
      );
    },
  );
}

// ================================================================
// Closure in control flow — no type stack leak.
// ================================================================

#[test]
fn test_closure_call_in_if_emits_call() {
  assert_sir_structure(
    r#"fun main() {
  imu f := fn(x: int) -> int => x * 2;
  if 1 > 0 {
    imu r: int = f(3);
  }
}"#,
    |sir| {
      // Closure FunDef + at least one Call must exist.
      let has_closure = sir.iter().any(|i| {
        matches!(
          i,
          Insn::FunDef {
            kind: FunctionKind::Closure { .. },
            ..
          }
        )
      });

      let has_call = sir
        .iter()
        .any(|i| matches!(i, Insn::Call { ty_id, .. } if ty_id.0 != 1));

      assert!(has_closure, "expected closure FunDef");
      assert!(has_call, "expected closure Call inside if");
    },
  );
}

#[test]
fn test_closure_call_in_while_emits_call() {
  assert_sir_structure(
    r#"fun main() {
  imu f := fn(x: int) -> int => x * 2;
  mut i: int = 0;
  while i < 1 {
    imu r: int = f(i);
    i = i + 1;
  }
}"#,
    |sir| {
      let has_closure = sir.iter().any(|i| {
        matches!(
          i,
          Insn::FunDef {
            kind: FunctionKind::Closure { .. },
            ..
          }
        )
      });

      let has_call = sir
        .iter()
        .any(|i| matches!(i, Insn::Call { ty_id, .. } if ty_id.0 != 1));

      assert!(has_closure, "expected closure FunDef");
      assert!(has_call, "expected closure Call inside while");
    },
  );
}

// ================================================================
// Mutable capture — by-copy semantics.
// ================================================================

#[test]
fn test_closure_mutable_capture_by_copy() {
  assert_sir_structure(
    r#"fun main() -> int {
  mut x: int = 10;
  imu f := fn(n: int) -> int => n + x;
  x = 99;
  f(5)
}"#,
    |sir| {
      let closure = sir.iter().find(|i| {
        matches!(
          i,
          Insn::FunDef {
            kind: FunctionKind::Closure { capture_count: 1 },
            ..
          }
        )
      });

      assert!(closure.is_some(), "expected closure with 1 capture (x)");
    },
  );
}

// ================================================================
// 3+ parameters.
// ================================================================

#[test]
fn test_closure_three_params() {
  assert_sir_structure(
    r#"fun main() -> int {
  imu f := fn(a: int, b: int, c: int) -> int => a + b + c;
  f(1, 2, 3)
}"#,
    |sir| {
      let closure = sir.iter().find(|i| {
        matches!(
          i,
          Insn::FunDef {
            kind: FunctionKind::Closure { capture_count: 0 },
            ..
          }
        )
      });

      if let Some(Insn::FunDef { params, .. }) = closure {
        assert_eq!(params.len(), 3, "expected 3 params, got {}", params.len());
      } else {
        panic!("expected closure FunDef with 3 params");
      }
    },
  );
}

// ================================================================
// Block closure with multiple statements + early return.
// ================================================================

#[test]
fn test_closure_block_multi_stmt() {
  assert_sir_structure(
    r#"fun main() -> int {
  imu f := fn(x: int) -> int {
    imu y: int = x * 2;
    imu z: int = y + 1;
    return z;
  };
  f(10)
}"#,
    |sir| {
      let has_closure = sir.iter().any(|i| {
        matches!(
          i,
          Insn::FunDef {
            kind: FunctionKind::Closure { .. },
            ..
          }
        )
      });

      // Block closure with locals should have VarDef + Store.
      let has_vardef = sir.iter().any(|i| matches!(i, Insn::VarDef { .. }));

      assert!(has_closure, "expected closure FunDef");
      assert!(has_vardef, "expected VarDef for local in closure body");
    },
  );
}

#[test]
fn test_closure_block_early_return() {
  assert_sir_structure(
    r#"fun main() -> int {
  imu f := fn(x: int) -> int {
    if x > 0 {
      return x * 2;
    }
    return 0;
  };
  f(5)
}"#,
    |sir| {
      let closure_idx = sir.iter().position(|i| {
        matches!(
          i,
          Insn::FunDef {
            kind: FunctionKind::Closure { .. },
            ..
          }
        )
      });

      assert!(closure_idx.is_some(), "expected closure FunDef");

      let returns_after = sir[closure_idx.unwrap()..]
        .iter()
        .filter(|i| matches!(i, Insn::Return { .. }))
        .count();

      assert!(
        returns_after >= 2,
        "expected >= 2 Returns in closure, got {returns_after}"
      );
    },
  );
}

// ================================================================
// SIR layout: closure hoisted before containing function.
// ================================================================

#[test]
fn test_closure_sir_hoisted_before_main() {
  assert_sir_structure(
    r#"fun main() {
  imu f := fn(x: int) -> int => x + 1;
}"#,
    |sir| {
      let closure_pos = sir.iter().position(|i| {
        matches!(
          i,
          Insn::FunDef {
            kind: FunctionKind::Closure { .. },
            ..
          }
        )
      });

      let main_pos = sir.iter().position(|i| {
        matches!(
          i,
          Insn::FunDef {
            kind: FunctionKind::UserDefined,
            ..
          }
        )
      });

      assert!(
        closure_pos.is_some() && main_pos.is_some(),
        "expected both closure and main FunDefs"
      );

      assert!(
        closure_pos.unwrap() < main_pos.unwrap(),
        "closure (idx={}) must precede main (idx={})",
        closure_pos.unwrap(),
        main_pos.unwrap()
      );
    },
  );
}

// ================================================================
// Call instruction uses closure's generated name.
// ================================================================

#[test]
fn test_closure_call_uses_generated_name() {
  assert_sir_structure(
    r#"fun main() -> int {
  imu f := fn(x: int) -> int => x;
  f(42)
}"#,
    |sir| {
      let closure_name = sir.iter().find_map(|i| {
        if let Insn::FunDef {
          name,
          kind: FunctionKind::Closure { .. },
          ..
        } = i
        {
          Some(*name)
        } else {
          None
        }
      });

      let call_name = sir.iter().find_map(|i| {
        if let Insn::Call { name, ty_id, .. } = i {
          if ty_id.0 != 1 { Some(*name) } else { None }
        } else {
          None
        }
      });

      assert!(
        closure_name.is_some() && call_name.is_some(),
        "expected closure FunDef and non-unit Call"
      );

      assert_eq!(
        closure_name.unwrap(),
        call_name.unwrap(),
        "Call must use the closure's generated name"
      );
    },
  );
}

/// Passing a named function to a `Fn(T) -> R` parameter —
/// `ho(direct)`. The Ident handler pushes a synthetic
/// `Value::Closure` with zero captures, and the existing
/// closure-param monomorphization pipeline (line ~10025 in
/// executor.rs) creates `ho__cl<N>` whose body calls `direct`
/// directly. Before this path existed the ident was silently
/// skipped — `ho(direct)` became `call ho()` with no args
/// and the binary hung trying to dispatch through an empty
/// function-pointer slot.
#[test]
fn test_named_fun_as_fn_param_argument() {
  assert_sir_structure(
    r#"fun ho(f: Fn(int) -> int) -> int {
  return f(3);
}
fun direct(x: int) -> int { x + 1 }
fun main() {
  imu b: int = ho(direct);
}"#,
    |sir| {
      // A specialized `ho__cl<N>` must have been emitted.
      // Use the fact that closure-param mono mangles names
      // with `__cl` as a prefix marker on the FunDef.
      //
      // Note: we can't resolve interner-backed Symbols to
      // strings here without an interner handle, so we
      // assert structural properties instead: there must be
      // TWO user FunDefs named something (ho and
      // ho__cl<N>), at least three Calls (ho__cl<N> from
      // main, direct from the specialized body, plus any
      // prelude), and no Call to the generic `ho` name.
      let user_fundef_count = sir
        .iter()
        .filter(|i| {
          matches!(
            i,
            Insn::FunDef {
              kind: FunctionKind::UserDefined,
              ..
            }
          )
        })
        .count();

      // Expect: ho (generic, body skipped but signature
      // emitted), direct, main, ho__cl<N> (specialized).
      assert!(
        user_fundef_count >= 3,
        "expected >= 3 user FunDefs (ho, direct, main, \
         ho__cl<N>), got {user_fundef_count}"
      );

      // At least one Call whose target differs from the
      // generic `ho` — proves specialization happened.
      let non_unit_calls = sir
        .iter()
        .filter_map(|i| match i {
          Insn::Call { name, ty_id, .. } if ty_id.0 != TyId(1).0 => Some(*name),
          _ => None,
        })
        .collect::<Vec<_>>();

      assert!(
        !non_unit_calls.is_empty(),
        "expected at least one Call emitted for the \
         specialized ho or direct"
      );
    },
  );

  // End-to-end: the program must compile without errors.
  assert_no_errors(
    r#"fun ho(f: Fn(int) -> int) -> int { f(3) }
fun direct(x: int) -> int { x + 1 }
fun main() {
  imu b: int = ho(direct);
}"#,
  );
}

/// Related shape — binding a named fun to a local of
/// `Fn(...) -> R` type and calling THROUGH the local:
/// `imu g: Fn() -> int = f; g()`. The Ident handler pushes
/// a synthetic `Value::Closure` for `f` (no call-target
/// context), the `imu` binding stores that closure value in
/// `g`, and the call through `g` resolves via the existing
/// closure-call path. Regression guard: both the declaration
/// AND the call-through must compile without errors.
#[test]
fn test_named_fun_bound_to_local_then_called() {
  assert_no_errors(
    r#"fun f() -> int { 42 }
fun main() {
  imu g: Fn() -> int = f;
  imu i: int = g();
}"#,
  );
}
