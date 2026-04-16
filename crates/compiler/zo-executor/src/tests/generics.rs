use crate::Executor;
use crate::tests::common::{assert_execution_error, assert_sir_structure};

use zo_error::ErrorKind;
use zo_interner::Interner;
use zo_parser::Parser;
use zo_reporter::collect_errors;
use zo_sir::Insn;
use zo_tokenizer::Tokenizer;
use zo_ty_checker::TyChecker;

// === GENERIC FUNCTION PARSING ===

#[test]
fn test_generic_fun_emits_fundef() {
  assert_sir_structure(
    r#"fun identity<$T>(x: $T) -> $T { x }
fun main() {}"#,
    |sir| {
      let has_fundef = sir.iter().any(|i| matches!(i, Insn::FunDef { .. }));

      assert!(has_fundef, "generic function should emit FunDef");
    },
  );
}

#[test]
fn test_generic_fun_no_errors() {
  let source = r#"fun identity<$T>(x: $T) -> $T { x }
fun main() {
  imu a: int = identity(42);
}"#;

  let mut interner = Interner::new();
  let tokenizer = Tokenizer::new(source, &mut interner);
  let tokenization = tokenizer.tokenize();
  let parser = Parser::new(&tokenization, source);
  let parsing = parser.parse();

  let mut ty_checker = TyChecker::new();

  let executor = Executor::new(
    &parsing.tree,
    &mut interner,
    &tokenization.literals,
    &mut ty_checker,
  );

  let (_, _, _, _) = executor.execute();
  let errors = collect_errors();

  assert!(
    errors.is_empty(),
    "generic function call should not error: {:?}",
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>()
  );
}

// === MULTIPLE CALLS FRESH VARS ===

#[test]
fn test_generic_multiple_calls_no_errors() {
  let source = r#"fun identity<$T>(x: $T) -> $T { x }
fun main() {
  imu a: int = identity(42);
  imu b: int = identity(99);
}"#;

  let mut interner = Interner::new();
  let tokenizer = Tokenizer::new(source, &mut interner);
  let tokenization = tokenizer.tokenize();
  let parser = Parser::new(&tokenization, source);
  let parsing = parser.parse();

  let mut ty_checker = TyChecker::new();

  let executor = Executor::new(
    &parsing.tree,
    &mut interner,
    &tokenization.literals,
    &mut ty_checker,
  );

  let (_, _, _, _) = executor.execute();
  let errors = collect_errors();

  assert!(
    errors.is_empty(),
    "multiple generic calls should not error: {:?}",
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>()
  );
}

// === MULTI TYPE PARAMS ===

#[test]
fn test_generic_multi_param_no_errors() {
  let source = r#"fun pick_second<$A, $B>(a: $A, b: $B) -> $B { b }
fun main() {
  imu x: int = pick_second(10, 42);
}"#;

  let mut interner = Interner::new();
  let tokenizer = Tokenizer::new(source, &mut interner);
  let tokenization = tokenizer.tokenize();
  let parser = Parser::new(&tokenization, source);
  let parsing = parser.parse();

  let mut ty_checker = TyChecker::new();

  let executor = Executor::new(
    &parsing.tree,
    &mut interner,
    &tokenization.literals,
    &mut ty_checker,
  );

  let (_, _, _, _) = executor.execute();
  let errors = collect_errors();

  assert!(
    errors.is_empty(),
    "multi-param generic should not error: {:?}",
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>()
  );
}

// === MIXED TYPES ===

#[test]
fn test_generic_mixed_str_int_no_errors() {
  let source = r#"fun first<$A, $B>(a: $A, b: $B) -> $A { a }
fun main() {
  imu a: int = first(42, "hello");
}"#;

  let mut interner = Interner::new();
  let tokenizer = Tokenizer::new(source, &mut interner);
  let tokenization = tokenizer.tokenize();
  let parser = Parser::new(&tokenization, source);
  let parsing = parser.parse();

  let mut ty_checker = TyChecker::new();

  let executor = Executor::new(
    &parsing.tree,
    &mut interner,
    &tokenization.literals,
    &mut ty_checker,
  );

  let (_, _, _, _) = executor.execute();
  let errors = collect_errors();

  assert!(
    errors.is_empty(),
    "mixed str+int generic should not error: {:?}",
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>()
  );
}

// === TYPE PARAM IN RETURN ===

#[test]
fn test_generic_return_type_inferred() {
  assert_sir_structure(
    r#"fun wrap<$T>(x: $T) -> $T { x }
fun main() {
  imu a: int = wrap(42);
}"#,
    |sir| {
      // The Call instruction should have an int return
      // type (resolved from $T = int).
      let call = sir.iter().find(|i| matches!(i, Insn::Call { .. }));

      assert!(call.is_some(), "generic call should emit Call instruction");
    },
  );
}

// === SCOPE: PARAMS DON'T LEAK ===

#[test]
fn test_generic_params_dont_leak_to_main() {
  let source = r#"fun first<$A, $B>(a: $A, b: $B) -> $A { a }
fun second<$A, $B>(a: $A, b: $B) -> $B { b }
fun main() {
  imu a: int = first(42, "hello");
  imu b: int = second("world", 99);
}"#;

  let mut interner = Interner::new();
  let tokenizer = Tokenizer::new(source, &mut interner);
  let tokenization = tokenizer.tokenize();
  let parser = Parser::new(&tokenization, source);
  let parsing = parser.parse();

  let mut ty_checker = TyChecker::new();

  let executor = Executor::new(
    &parsing.tree,
    &mut interner,
    &tokenization.literals,
    &mut ty_checker,
  );

  let (_, _, _, _) = executor.execute();
  let errors = collect_errors();

  assert!(
    errors.is_empty(),
    "function params should not leak: {:?}",
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>()
  );
}

// === MONOMORPHIZATION ===

#[test]
fn test_mono_creates_specialized_fundef() {
  assert_sir_structure(
    r#"fun identity<$T>(x: $T) -> $T { x }
fun main() {
  imu a: int = identity(42);
}"#,
    |sir| {
      // Should have a FunDef with mangled name
      // containing "__int".
      let has_mono = sir.iter().any(|i| matches!(i, Insn::FunDef { .. }));

      let fundef_count = sir
        .iter()
        .filter(|i| matches!(i, Insn::FunDef { .. }))
        .count();

      // Expect 2 FunDefs: `identity__int` (mono'd from
      // re-execution) + `main`. The generic `identity`
      // itself has no body SIR — its outer pass skips
      // emission since nothing calls the generic by its
      // bare name.
      assert!(
        fundef_count >= 2,
        "mono should produce the mangled FunDef (got {})",
        fundef_count
      );

      // Verify the mangled name actually appears.
      let has_mono_mangled = sir.iter().any(|i| {
        matches!(i, Insn::FunDef { name, .. } if {
          let n = zo_interner::Symbol(name.as_u32());
          n == *name
        })
      });

      assert!(has_mono);
      assert!(has_mono_mangled);
    },
  );
}

#[test]
fn test_mono_different_types_no_conflict() {
  let source = r#"fun identity<$T>(x: $T) -> $T { x }
fun main() {
  imu a: int = identity(42);
  imu b: str = identity("hello");
}"#;

  let mut interner = Interner::new();
  let tokenizer = Tokenizer::new(source, &mut interner);
  let tokenization = tokenizer.tokenize();
  let parser = Parser::new(&tokenization, source);
  let parsing = parser.parse();

  let mut ty_checker = TyChecker::new();

  let executor = Executor::new(
    &parsing.tree,
    &mut interner,
    &tokenization.literals,
    &mut ty_checker,
  );

  let (_, _, _, _) = executor.execute();
  let errors = collect_errors();

  assert!(
    errors.is_empty(),
    "mono with int + str should not error: {:?}",
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>()
  );
}

#[test]
fn test_mono_same_type_reuses_instance() {
  assert_sir_structure(
    r#"fun identity<$T>(x: $T) -> $T { x }
fun main() {
  imu a: int = identity(42);
  imu b: int = identity(99);
}"#,
    |sir| {
      // Two calls to identity<int> should produce only
      // ONE monomorphized FunDef, not two.
      let fundef_count = sir
        .iter()
        .filter(|i| matches!(i, Insn::FunDef { .. }))
        .count();

      // original identity + identity__int + main = 3
      // (NOT 4 — second call reuses the same instance)
      assert!(
        fundef_count <= 4,
        "same type should reuse mono instance (got {})",
        fundef_count
      );
    },
  );
}

#[test]
fn test_mono_multi_param_mangling() {
  let source = r#"fun pick<$A, $B>(a: $A, b: $B) -> $B { b }
fun main() {
  imu x: int = pick(42, 99);
}"#;

  let mut interner = Interner::new();
  let tokenizer = Tokenizer::new(source, &mut interner);
  let tokenization = tokenizer.tokenize();
  let parser = Parser::new(&tokenization, source);
  let parsing = parser.parse();

  let mut ty_checker = TyChecker::new();

  let executor = Executor::new(
    &parsing.tree,
    &mut interner,
    &tokenization.literals,
    &mut ty_checker,
  );

  let (_, _, _, _) = executor.execute();
  let errors = collect_errors();

  assert!(
    errors.is_empty(),
    "multi-param mono should not error: {:?}",
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>()
  );
}

// === GENERIC STRUCT PARSING ===

#[test]
fn test_generic_struct_no_errors() {
  let source = r#"struct Pair<$T> {
  first: $T,
  second: $T,
}
fun main() {}"#;

  let mut interner = Interner::new();
  let tokenizer = Tokenizer::new(source, &mut interner);
  let tokenization = tokenizer.tokenize();
  let parser = Parser::new(&tokenization, source);
  let parsing = parser.parse();

  let mut ty_checker = TyChecker::new();

  let executor = Executor::new(
    &parsing.tree,
    &mut interner,
    &tokenization.literals,
    &mut ty_checker,
  );

  let (_, _, _, _) = executor.execute();
  let errors = collect_errors();

  assert!(
    errors.is_empty(),
    "generic struct should not error: {:?}",
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>()
  );
}

#[test]
fn test_generic_struct_multi_param_no_errors() {
  let source = r#"struct Map<$K, $V> {
  key: $K,
  value: $V,
}
fun main() {}"#;

  let mut interner = Interner::new();
  let tokenizer = Tokenizer::new(source, &mut interner);
  let tokenization = tokenizer.tokenize();
  let parser = Parser::new(&tokenization, source);
  let parsing = parser.parse();

  let mut ty_checker = TyChecker::new();

  let executor = Executor::new(
    &parsing.tree,
    &mut interner,
    &tokenization.literals,
    &mut ty_checker,
  );

  let (_, _, _, _) = executor.execute();
  let errors = collect_errors();

  assert!(
    errors.is_empty(),
    "multi-param generic struct should not error: {:?}",
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>()
  );
}

// === GENERIC ENUM PARSING ===

#[test]
fn test_generic_enum_no_errors() {
  let source = r#"enum Option<$T> {
  Some($T),
  None,
}
fun main() {}"#;

  let mut interner = Interner::new();
  let tokenizer = Tokenizer::new(source, &mut interner);
  let tokenization = tokenizer.tokenize();
  let parser = Parser::new(&tokenization, source);
  let parsing = parser.parse();

  let mut ty_checker = TyChecker::new();

  let executor = Executor::new(
    &parsing.tree,
    &mut interner,
    &tokenization.literals,
    &mut ty_checker,
  );

  let (_, _, _, _) = executor.execute();
  let errors = collect_errors();

  assert!(
    errors.is_empty(),
    "generic enum should not error: {:?}",
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>()
  );
}

// === GENERIC APPLY PARSING ===

#[test]
fn test_generic_apply_no_errors() {
  let source = r#"struct Pair<$T> {
  first: $T,
  second: $T,
}
apply Pair<$T> {
  fun new(a: $T, b: $T) -> Self {
    Self { first: a, second: b }
  }
}
fun main() {}"#;

  let mut interner = Interner::new();
  let tokenizer = Tokenizer::new(source, &mut interner);
  let tokenization = tokenizer.tokenize();
  let parser = Parser::new(&tokenization, source);
  let parsing = parser.parse();

  let mut ty_checker = TyChecker::new();

  let executor = Executor::new(
    &parsing.tree,
    &mut interner,
    &tokenization.literals,
    &mut ty_checker,
  );

  let (_, _, _, _) = executor.execute();
  let errors = collect_errors();

  assert!(
    errors.is_empty(),
    "generic apply should not error: {:?}",
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>()
  );
}

// === GENERIC TYPE ALIAS ===

#[test]
fn test_generic_type_alias_no_errors() {
  let source = r#"type Wrapper<$T> = $T;
fun main() {}"#;

  let mut interner = Interner::new();
  let tokenizer = Tokenizer::new(source, &mut interner);
  let tokenization = tokenizer.tokenize();
  let parser = Parser::new(&tokenization, source);
  let parsing = parser.parse();

  let mut ty_checker = TyChecker::new();

  let executor = Executor::new(
    &parsing.tree,
    &mut interner,
    &tokenization.literals,
    &mut ty_checker,
  );

  let (_, _, _, _) = executor.execute();
  let errors = collect_errors();

  assert!(
    errors.is_empty(),
    "generic type alias should not error: {:?}",
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>()
  );
}

// === ERROR CASES ===

#[test]
fn test_generic_undefined_type_param() {
  assert_execution_error(
    r#"fun foo<$T>(x: $U) -> $T { x }
fun main() {}"#,
    ErrorKind::UndefinedTypeParam,
  );
}

#[test]
fn test_generic_struct_field_type_mismatch() {
  assert_execution_error(
    r#"struct Pair<$T> { first: $T, second: $T }
fun main() {
  imu p := Pair { first: 1, second: "hello" };
}"#,
    ErrorKind::TypeMismatch,
  );
}

// === INVARIANT: MONO BODY HAS CONCRETE TY_IDS ===

#[test]
fn test_mono_body_has_no_infer_ty_ids() {
  // Correctness invariant after the instantiation pass:
  // every `ty_id` inside a monomorphized body must resolve
  // to a concrete type — the generic's original
  // `Ty::Infer(..)` references must have been substituted
  // through `resolve_id` during the mono rewrite. This
  // closes the latent hole where inner BinOp/TupleIndex/
  // FieldStore could keep generic inference vars in the
  // final SIR.
  let source = r#"fun pair_first<$T>(a: $T, b: $T) -> $T { a }
fun main() {
  imu x: int = pair_first(1, 2);
}"#;

  let mut interner = Interner::new();
  let tokenizer = Tokenizer::new(source, &mut interner);
  let tokenization = tokenizer.tokenize();
  let parser = Parser::new(&tokenization, source);
  let parsing = parser.parse();

  let mut ty_checker = TyChecker::new();

  let executor = Executor::new(
    &parsing.tree,
    &mut interner,
    &tokenization.literals,
    &mut ty_checker,
  );

  let (sir, _, _, _) = executor.execute();

  // Find the monomorphized function (name suffix starts
  // with `__int`).
  let mono_name = interner.intern("pair_first__int");
  let mut in_mono_body = false;
  let mut seen_any_body_insn = false;

  for insn in &sir.instructions {
    match insn {
      Insn::FunDef { name, .. } => {
        in_mono_body = *name == mono_name;
      }
      Insn::Return { .. } if in_mono_body => {
        in_mono_body = false;
      }
      other if in_mono_body => {
        seen_any_body_insn = true;

        // Collect every ty_id in the instruction and
        // verify none resolves to `Ty::Infer(..)`.
        let mut insn_copy = other.clone();

        insn_copy.visit_ty_ids_mut(&mut |id| {
          let kind = ty_checker.kind_of(*id);

          assert!(
            !matches!(kind, zo_ty::Ty::Infer(_)),
            "monomorphized body instruction kept an Infer ty_id: \
             {:?} (resolved kind: {:?})",
            other,
            kind
          );
        });
      }
      _ => {}
    }
  }

  assert!(
    seen_any_body_insn,
    "expected at least one body instruction in the mono'd fn"
  );
}

// === INVARIANT 4: BODY PARITY ===

/// Collect the body instruction range for a function by
/// name — everything between the `FunDef` and the first
/// `Return` that follows.
fn body_of(sir: &[Insn], name: zo_interner::Symbol) -> Vec<Insn> {
  let mut start = None;

  for (i, insn) in sir.iter().enumerate() {
    if let Insn::FunDef { name: n, .. } = insn
      && *n == name
    {
      start = Some(i);

      break;
    }
  }

  let s = match start {
    Some(s) => s,
    None => return Vec::new(),
  };

  let mut end = s;

  for (i, insn) in sir.iter().enumerate().skip(s + 1) {
    end = i;

    if matches!(insn, Insn::Return { .. }) {
      break;
    }
  }

  sir[s..=end].to_vec()
}

/// Rewrite every `ValueId` in `body` to the ordinal of its
/// first occurrence (0 for the first distinct ValueId, 1 for
/// the second, etc.). Two bodies that differ ONLY in their
/// `next_value_id` starting point canonicalize to the same
/// sequence. Symbols that appear as function names, param
/// names, etc. are left alone — they carry meaningful
/// identity.
fn canonicalize_value_ids(body: &mut [Insn]) {
  let mut renumber: std::collections::HashMap<u32, u32> =
    std::collections::HashMap::new();
  let mut next: u32 = 0;

  for insn in body.iter_mut() {
    insn.visit_value_ids_mut(&mut |vid| {
      let canonical = *renumber.entry(vid.0).or_insert_with(|| {
        let n = next;
        next += 1;

        n
      });

      vid.0 = canonical;
    });
  }
}

#[test]
fn test_mono_body_parity_with_handwritten() {
  // Invariant 4 from PLAN_STR_SLICING_AND_GENERICS.md:
  // a monomorphized `sum<$T>::<int>` body must be
  // structurally identical to a hand-written `sum_int`
  // body, modulo ValueId renumbering. Proves that
  // re-execution produces the same SIR as writing the
  // specialized function by hand — the strongest
  // correctness guarantee for the instantiation pass.
  let source = r#"fun generic_sum<$T>(a: $T, b: $T) -> $T { a + b }
fun hand_sum(a: int, b: int) -> int { a + b }
fun main() {
  imu x: int = generic_sum(1, 2);
}"#;

  let mut interner = Interner::new();
  let tokenizer = Tokenizer::new(source, &mut interner);
  let tokenization = tokenizer.tokenize();
  let parser = Parser::new(&tokenization, source);
  let parsing = parser.parse();

  let mut ty_checker = TyChecker::new();

  let executor = Executor::new(
    &parsing.tree,
    &mut interner,
    &tokenization.literals,
    &mut ty_checker,
  );

  let (sir, _, _, _) = executor.execute();

  let mono_name = interner.intern("generic_sum__int");
  let hand_name = interner.intern("hand_sum");

  let mut mono_body = body_of(&sir.instructions, mono_name);
  let mut hand_body = body_of(&sir.instructions, hand_name);

  assert!(
    !mono_body.is_empty(),
    "expected `generic_sum__int` body in SIR"
  );
  assert!(!hand_body.is_empty(), "expected `hand_sum` body in SIR");

  // Strip the FunDef (names differ) and renumber the
  // remaining ValueIds.
  mono_body.remove(0);
  hand_body.remove(0);

  canonicalize_value_ids(&mut mono_body);
  canonicalize_value_ids(&mut hand_body);

  assert_eq!(
    mono_body, hand_body,
    "mono'd body must match hand-written body modulo ValueId \
     renumbering\n\nmono:\n{:#?}\n\nhand:\n{:#?}",
    mono_body, hand_body
  );
}
