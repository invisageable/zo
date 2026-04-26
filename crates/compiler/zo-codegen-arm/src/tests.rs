pub(crate) mod common;
pub(crate) mod concurrency;
pub(crate) mod errors;
pub(crate) mod templates;

use crate::ARM64Gen;
use zo_value::{FunctionKind, Pubness};

use zo_executor::Executor;
use zo_interner::Interner;
use zo_parser::Parser;
use zo_sir::{Insn, Sir};
use zo_tokenizer::Tokenizer;
use zo_ty::TyId;
use zo_ty_checker::TyChecker;
use zo_value::ValueId;

#[test]
fn test_complete_pipeline_hello_world() {
  let source = r#"fun main() { show("hello world") }"#;

  let mut interner = Interner::new();
  let tokenizer = Tokenizer::new(source, &mut interner);
  let tokenization = tokenizer.tokenize();

  let parser = Parser::new(&tokenization, source);
  let parsing_result = parser.parse();

  let mut ty_checker = TyChecker::new();

  let executor = Executor::new(
    &parsing_result.tree,
    &mut interner,
    &tokenization.literals,
    &mut ty_checker,
  );

  let (sir, _annotations, _, _) = executor.execute();

  assert_eq!(sir.instructions.len(), 4);

  if let Insn::FunDef {
    name,
    params,
    body_start,
    ..
  } = &sir.instructions[0]
  {
    assert_eq!(interner.get(*name), "main");
    assert_eq!(params.len(), 0);
    assert_eq!(*body_start, 1);
  } else {
    panic!("Expected FunDef instruction");
  }

  if let Insn::ConstString { symbol, .. } = &sir.instructions[1] {
    assert_eq!(interner.get(*symbol), "hello world");
  } else {
    panic!("Expected ConstString instruction");
  }

  if let Insn::Call { name, args, .. } = &sir.instructions[2] {
    assert_eq!(interner.get(*name), "show");
    assert_eq!(args.len(), 1);
  } else {
    panic!("Expected Call instruction");
  }

  assert!(matches!(
    sir.instructions[3],
    Insn::Return { value: None, .. }
  ));

  let mut codegen = ARM64Gen::new(&interner);
  let artifact = codegen.generate(&sir);

  assert!(!artifact.code.is_empty());
  // Should have at least: MOV X16, MOV X0, ADR X1, MOV X2, SVC (5 instructions
  // * 4 bytes) Plus exit syscall: MOV X16, MOV X0, SVC (3 more instructions)
  // Total: 8 instructions * 4 bytes = 32 bytes minimum
  assert!(artifact.code.len() >= 32);

  let hello_bytes = b"hello world\0";

  let code_contains_string = artifact
    .code
    .windows(hello_bytes.len())
    .any(|window| window == hello_bytes);

  assert!(
    code_contains_string,
    "Generated code should contain the string 'hello world'"
  );
}

// ================================================================
// Closure codegen integration tests.
// ================================================================

/// Helper: run full pipeline (tokenize → parse → execute → codegen)
/// and return the generated artifact's code bytes.
fn compile_to_code(source: &str) -> Vec<u8> {
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

  let mut codegen = ARM64Gen::new(&interner);
  let artifact = codegen.generate(&sir);

  artifact.code
}

#[test]
fn test_closure_generates_code() {
  let code = compile_to_code(
    r#"fun main() {
  imu f := fn(x: int) -> int => x * 2;
}"#,
  );

  // Closure + main: at minimum prologue + epilogue
  // for each (2 functions × ~3 insns × 4 bytes).
  assert!(code.len() >= 24, "expected >= 24 bytes, got {}", code.len());
}

#[test]
fn test_closure_with_call_generates_bl() {
  let code = compile_to_code(
    r#"fun main() -> int {
  imu f := fn(x: int) -> int => x + 1;
  f(5)
}"#,
  );

  // BL instruction: 0b100101_xxxxxx = 0x94xxxxxx or 0x97xxxxxx.
  // Scan for at least one BL in the generated code.
  let has_bl = code.chunks_exact(4).any(|chunk| {
    let insn = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);

    (insn >> 26) == 0b100101
  });

  assert!(has_bl, "expected at least one BL instruction");
}

#[test]
fn test_closure_forward_ref_patched() {
  // Closure is hoisted before main in SIR. The Call
  // inside main is a forward reference that gets patched.
  // After patching, the BL offset should be negative
  // (jumping backward to the closure).
  let code = compile_to_code(
    r#"fun main() -> int {
  imu f := fn(x: int) -> int => x;
  f(42)
}"#,
  );

  // Find BL instructions and check at least one has a
  // negative offset (backward jump to closure).
  let has_backward_bl = code.chunks_exact(4).any(|chunk| {
    let insn = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);

    if (insn >> 26) == 0b100101 {
      // imm26 is sign-extended.
      let imm26 = insn & 0x03FF_FFFF;

      // Bit 25 set means negative offset.
      imm26 & (1 << 25) != 0
    } else {
      false
    }
  });

  assert!(
    has_backward_bl,
    "expected backward BL (closure defined before main)"
  );
}

#[test]
fn test_closure_float_param_spill() {
  // Float closure: param arrives in D0, must be spilled
  // with STR Dt (FP store), not STR Xt (GP store).
  let code = compile_to_code(
    r#"fun main() {
  imu f := fn(x: float) -> float => x;
}"#,
  );

  // STR Dt, [Xn, #imm]: 1111_1101_00xx (0xFD0x).
  // At least one FP store in the closure prologue.
  let has_fp_str = code.chunks_exact(4).any(|chunk| {
    let insn = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);

    // STR (FP, unsigned offset): top 10 bits = 1111110100.
    (insn >> 22) == 0b1111110100
  });

  assert!(has_fp_str, "expected FP STR for float param spill");
}

#[test]
fn test_closure_multi_param_generates_code() {
  let code = compile_to_code(
    r#"fun main() -> int {
  imu f := fn(a: int, b: int, c: int) -> int => a + b + c;
  f(1, 2, 3)
}"#,
  );

  assert!(
    code.len() >= 32,
    "expected >= 32 bytes for 3-param closure, got {}",
    code.len()
  );
}

#[test]
fn test_closure_capture_generates_code() {
  let code = compile_to_code(
    r#"fun main() -> int {
  imu y: int = 10;
  imu f := fn(x: int) -> int => x + y;
  f(5)
}"#,
  );

  assert!(
    code.len() >= 32,
    "expected >= 32 bytes for closure with capture, got {}",
    code.len()
  );
}

#[test]
fn test_string_index_emits_ldrb() {
  let source = r#"fun main() {
  imu s: str = "hello";
  showln(s[0]);
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

  let mut codegen = ARM64Gen::new(&interner);
  let artifact = codegen.generate(&sir);

  // LDRB unsigned-offset: bits [31:22] = 00_11_1001_01.
  let has_ldrb = artifact.code.chunks_exact(4).any(|c| {
    let insn = u32::from_le_bytes([c[0], c[1], c[2], c[3]]);

    (insn >> 22) == 0b0011100101
  });

  assert!(has_ldrb, "expected LDRB instruction for string indexing");
}

#[test]
fn test_string_index_check_eq_char() {
  // Matches minimal str-index.zo: check@eq(s[0], 'h')
  let source = r#"fun main() {
  imu s: str = "hello";
  check@eq(s[0], 'h');
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

  // Verify ArrayIndex has char type.
  let arr_idx = sir.instructions.iter().find_map(|i| {
    if let Insn::ArrayIndex { ty_id, .. } = i {
      Some(ty_id.0)
    } else {
      None
    }
  });

  assert_eq!(arr_idx, Some(3), "ArrayIndex should have char ty_id");

  // Verify BinOp Eq exists.
  let has_eq = sir.instructions.iter().any(|i| {
    matches!(
      i,
      Insn::BinOp {
        op: zo_sir::BinOp::Eq,
        ..
      }
    )
  });

  assert!(has_eq, "expected BinOp::Eq for check@eq");

  // Generate code — should not panic.
  let mut codegen = ARM64Gen::new(&interner);
  let artifact = codegen.generate(&sir);

  assert!(!artifact.code.is_empty());
}

// ================================================================
// Original codegen tests.
// ================================================================

#[test]
fn test_main_function_detection() {
  let mut interner = Interner::new();

  let mut sir = Sir::new();

  sir.emit(Insn::FunDef {
    name: interner.intern("main"),
    params: vec![],
    return_ty: TyId(1),
    body_start: 1,
    kind: FunctionKind::UserDefined,
    pubness: Pubness::No,
    mut_self: false,
  });
  sir.emit(Insn::Return {
    value: None,
    ty_id: TyId(1),
  });

  let mut codegen = ARM64Gen::new(&interner);
  let artifact = codegen.generate(&sir);

  assert!(!artifact.code.is_empty());
  // assert!(artifact.code.len() >= 12);
}

#[test]
fn test_string_fixup() {
  let mut interner = Interner::new();

  let mut sir = Sir::new();
  let main_sym = interner.intern("main");
  let show_sym = interner.intern("show");
  let hello_sym = interner.intern("hello");
  let world_sym = interner.intern("world");

  sir.emit(Insn::FunDef {
    name: main_sym,
    params: vec![],
    return_ty: TyId(1),
    body_start: 1,
    kind: FunctionKind::UserDefined,
    pubness: Pubness::No,
    mut_self: false,
  });

  sir.emit(Insn::ConstString {
    dst: ValueId(0),
    symbol: hello_sym,
    ty_id: TyId(4),
  });
  sir.emit(Insn::Call {
    dst: ValueId(1),
    name: show_sym,
    args: vec![ValueId(0)],
    ty_id: TyId(1),
  });
  sir.emit(Insn::ConstString {
    dst: ValueId(2),
    symbol: world_sym,
    ty_id: TyId(4),
  });
  sir.emit(Insn::Call {
    dst: ValueId(3),
    name: show_sym,
    args: vec![ValueId(2)],
    ty_id: TyId(1),
  });
  sir.emit(Insn::Return {
    value: None,
    ty_id: TyId(1),
  });

  let mut codegen = ARM64Gen::new(&interner);
  let artifact = codegen.generate(&sir);

  let hello_bytes = b"hello\0";
  let world_bytes = b"world\0";

  let code_contains_hello = artifact
    .code
    .windows(hello_bytes.len())
    .any(|window| window == hello_bytes);

  let code_contains_world = artifact
    .code
    .windows(world_bytes.len())
    .any(|window| window == world_bytes);

  assert!(code_contains_hello, "should contain 'hello'");
  assert!(code_contains_world, "should contain 'world'");
}

// ================================================================
// Enum pretty-printer (ZO-CL08) — tests the `register_enum_meta`
// + `is_enum_value` + `emit_enum_write` path that turns
// `show(Loot::Gold(50))` into a human-readable
// `Loot::Gold(...)` instead of leaking a stack pointer.
// ================================================================

/// True if `needle` appears as a contiguous byte sequence in
/// `haystack`. Used to assert that the enum pretty-printer
/// baked a display string into the final artifact.
fn code_contains(haystack: &[u8], needle: &[u8]) -> bool {
  haystack.windows(needle.len()).any(|w| w == needle)
}

#[test]
fn test_enum_tuple_variant_show_bakes_display_string() {
  // `show(Loot::Gold(50))` must emit a UTF-8 `"Loot::Gold"`
  // somewhere in the artifact — that's the pre-baked display
  // string the cmp-chain loads via ADR + fixup.
  let code = compile_to_code(
    r#"
enum Loot {
  Gold(int),
  Nothing,
}

fun main() {
  show(Loot::Gold(50));
}"#,
  );

  assert!(
    code_contains(&code, b"Loot::Gold"),
    "expected 'Loot::Gold' display string in the artifact",
  );
}

#[test]
fn test_enum_tuple_variant_show_bakes_parens() {
  // Tuple variants print actual payload values wrapped
  // in parentheses: `Loot::Gold(50)`.
  let code = compile_to_code(
    r#"
enum Loot {
  Gold(int),
  Nothing,
}

fun main() {
  show(Loot::Gold(50));
}"#,
  );

  assert!(
    code_contains(&code, b"(") && code_contains(&code, b")"),
    "expected '(' and ')' punctuation for the tuple variant",
  );
}

#[test]
fn test_enum_unit_variant_show_has_no_parens() {
  // An enum with only unit variants must not have
  // parenthesis punctuation in the artifact.
  let code = compile_to_code(
    r#"
enum Color {
  Red,
  Green,
  Blue,
}

fun main() {
  show(Color::Red);
}"#,
  );

  assert!(
    code_contains(&code, b"Color::Red"),
    "expected 'Color::Red' display string in the artifact",
  );
  assert!(
    !code_contains(&code, b"(...)"),
    "unit-only enums must not bake the '(...)' suffix",
  );
}

#[test]
fn test_enum_mixed_variants_bake_all_display_strings() {
  // An enum with both tuple and unit variants must bake one
  // display string per variant — the cmp-chain in
  // `emit_enum_write` references them by discriminant.
  let code = compile_to_code(
    r#"
enum Loot {
  Gold(int),
  Potion(int),
  Nothing,
}

fun main() {
  show(Loot::Gold(42));
}"#,
  );

  for variant in [b"Loot::Gold".as_slice(), b"Loot::Potion", b"Loot::Nothing"] {
    assert!(
      code_contains(&code, variant),
      "expected '{}' display string in the artifact",
      std::str::from_utf8(variant).unwrap(),
    );
  }
}

#[test]
fn test_enum_tuple_variant_emits_cmp_chain() {
  // One `CMP (immediate)` per variant — the cmp-chain is the
  // whole point of `emit_enum_write`. With three variants we
  // expect at least three `CMP immediate` instructions in the
  // artifact.
  //
  // CMP (immediate) 64-bit encoding: 0xF100_0000 top bits.
  let code = compile_to_code(
    r#"
enum Loot {
  Gold(int),
  Potion(int),
  Nothing,
}

fun main() {
  show(Loot::Gold(1));
}"#,
  );

  let cmp_imm_count = code
    .chunks_exact(4)
    .filter(|chunk| {
      let insn = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);

      // 64-bit CMP (immediate): SUBS XZR, Xn, #imm12
      // sf=1, S=1, op=101, 0xF1xx_xxxx with Rd=31.
      (insn & 0xFF00_0000) == 0xF100_0000 && (insn & 0x1F) == 0x1F
    })
    .count();

  assert!(
    cmp_imm_count >= 3,
    "expected at least 3 CMP instructions for a 3-variant enum, got {cmp_imm_count}",
  );
}

#[test]
fn test_enum_independent_from_string_ellipsis() {
  // Regression: the shared `"(...)"` symbol must not collide
  // with a user-interned string. Compile a program that prints
  // a literal `"(...)"` via `show`, alongside a tuple-variant
  // enum — both should survive.
  let code = compile_to_code(
    r#"
enum Loot {
  Gold(int),
  Nothing,
}

fun main() {
  show(Loot::Gold(1));
  show("(...)");
}"#,
  );

  // The ellipsis byte pattern must still appear; even if the
  // user-string path dedupes against the enum-owned symbol,
  // the displayed output is identical either way.
  assert!(code_contains(&code, b"(...)"));
  assert!(code_contains(&code, b"Loot::Gold"));
}

// ================================================================
// Generic enum payload pretty-printing — `enum_metas` is keyed by
// the generic enum's `TyId` and stores the template's `Ty::Infer
// ($T)` field types. `showln(Maybe<str>::Some("hi"))` used to
// dispatch the str payload through `emit_itoa_and_write` and leak
// the str header pointer (e.g. `Maybe::Some(4341608703)`). The fix
// captures the per-construction concrete payload types in
// `value_enum_field_tys` at `EnumConstruct` time and propagates
// them through `Store`/`Load`, so the writer dispatch sees `str`
// and emits an `SYS_WRITE` against the string payload instead of
// integer formatting.
// ================================================================

#[test]
fn test_generic_enum_str_payload_does_not_leak_pointer() {
  // The Some-arm payload print must take the str path (LDR
  // length + ADD #8 + SYS_WRITE), not the itoa path. The str
  // path always emits `0xD0000010` (`MOV X16, #SYS_WRITE`)
  // immediately after the payload-load LDR, while the itoa
  // path BLs into the runtime helper. Asserting on the runtime
  // string `"hi"` baked into the artifact is the most direct
  // observation that the str payload survived through codegen.
  let code = compile_to_code(
    r#"
enum Maybe<$T> {
  Some($T),
  None,
}

fun main() {
  imu m: Maybe<str> = Maybe::Some("hi");
  showln(m);
}"#,
  );

  assert!(
    code_contains(&code, b"Maybe::Some"),
    "expected 'Maybe::Some' display string in the artifact",
  );
  assert!(
    code_contains(&code, b"hi\0"),
    "expected the str payload `\"hi\"` to be baked into the \
     artifact — without the per-construction override the str's \
     header pointer would print as a number and `hi` would never \
     be referenced through SYS_WRITE",
  );
}

#[test]
fn test_generic_enum_int_then_str_dispatches_independently() {
  // Two `Maybe` instantiations in the same function — `<int>`
  // followed by `<str>`. The second construction's payload type
  // must override the first's; the artifact must contain the
  // str literal `"hi"` baked alongside the int payload format.
  let code = compile_to_code(
    r#"
enum Maybe<$T> {
  Some($T),
  None,
}

fun main() {
  imu m1: Maybe<int> = Maybe::Some(42);
  imu m2: Maybe<str> = Maybe::Some("hi");

  showln(m1);
  showln(m2);
}"#,
  );

  assert!(
    code_contains(&code, b"Maybe::Some"),
    "expected 'Maybe::Some' display string in the artifact",
  );
  assert!(
    code_contains(&code, b"hi\0"),
    "expected the str payload `\"hi\"` from the second \
     construction to be baked into the artifact",
  );
}

// ================================================================
// `emit_str_sp` slow-path scratch register — the SP-relative store
// helper falls back to a computed address when the offset overflows
// the inline-encodable range. Some call sites pass X16 as the value
// they want to store (it carries a freshly-built tag or payload
// word), so the slow path must pick a different register to hold
// the computed address — otherwise the address overwrites the
// value before STR consumes it and the slot ends up holding a
// pointer-shaped sentinel instead of the intended bits. This shows
// up at runtime as match arms whose discriminants are neither 0
// nor 1, so every arm's `BranchIfNot` skips and every match
// silently does nothing.
// ================================================================

#[test]
fn test_str_sp_slow_path_does_not_self_clobber() {
  // Drive a `Vec::get` SIR call at a struct-cursor that
  // overflows the inline STR-imm range. `emit_vec_get`
  // builds the Option aggregate by storing X16 (the value)
  // at SP-relative offsets — when the offset exceeds the
  // inline range, the slow path computes the address into a
  // scratch register before STR. Re-using X16 as both value
  // and scratch caused the address to overwrite the value;
  // the binary contained `str X16, [X16]` and the slot ended
  // up holding a pointer-shaped sentinel instead of the
  // intended discriminant. Manifested at runtime as match
  // arms whose discriminant compares were never satisfied —
  // every arm's `BranchIfNot` skipped, every match silently
  // did nothing.
  //
  // Padding: a `Vec::new` followed by 35 `HashMap::insert`s
  // — each call's scratch budget is hardcoded in
  // `zo-register-allocation` (1 + 2 * N slots for the Vec's
  // ptr field + the inserts' k/v scratch pairs), pushing the
  // ensuing `Vec::get`'s opt-base past 255 bytes from
  // `struct_base`.
  let mut interner = Interner::new();
  let main_sym = interner.intern("main");
  let vec_new_sym = interner.intern("Vec::new");
  let map_new_sym = interner.intern("HashMap::new");
  let map_insert_sym = interner.intern("HashMap::insert");
  let vec_get_sym = interner.intern("Vec::get");

  let mut sir = Sir::new();

  sir.emit(Insn::FunDef {
    name: main_sym,
    params: vec![],
    return_ty: TyId(1),
    body_start: 1,
    kind: FunctionKind::UserDefined,
    pubness: Pubness::No,
    mut_self: false,
  });

  let mut next: u32 = 0;
  let const_int = |sir: &mut Sir, next: &mut u32| -> ValueId {
    let v = ValueId(*next);
    *next += 1;
    sir.emit(Insn::ConstInt {
      dst: v,
      value: 0,
      ty_id: TyId(1),
    });
    v
  };
  let fresh = |next: &mut u32| -> ValueId {
    let v = ValueId(*next);
    *next += 1;
    v
  };

  // `Vec::new` / `HashMap::new` take three executor-injected
  // `(elem/key kind, sz, val_sz)` constants — seed each call
  // with three ConstInts so the codegen handler sees a full
  // arg list.
  let v_args = vec![
    const_int(&mut sir, &mut next),
    const_int(&mut sir, &mut next),
    const_int(&mut sir, &mut next),
  ];
  let v_handle = fresh(&mut next);

  sir.emit(Insn::Call {
    dst: v_handle,
    name: vec_new_sym,
    args: v_args,
    ty_id: TyId(1),
  });

  let m_args = vec![
    const_int(&mut sir, &mut next),
    const_int(&mut sir, &mut next),
    const_int(&mut sir, &mut next),
  ];
  let m_handle = fresh(&mut next);

  sir.emit(Insn::Call {
    dst: m_handle,
    name: map_new_sym,
    args: m_args,
    ty_id: TyId(1),
  });

  for _ in 0..35 {
    let k = const_int(&mut sir, &mut next);
    let val = const_int(&mut sir, &mut next);
    let dst = fresh(&mut next);

    sir.emit(Insn::Call {
      dst,
      name: map_insert_sym,
      args: vec![m_handle, k, val],
      ty_id: TyId(1),
    });
  }

  let idx = const_int(&mut sir, &mut next);
  let get_dst = fresh(&mut next);

  sir.emit(Insn::Call {
    dst: get_dst,
    name: vec_get_sym,
    args: vec![v_handle, idx],
    ty_id: TyId(1),
  });
  sir.emit(Insn::Return {
    value: None,
    ty_id: TyId(1),
  });

  sir.next_value_id = next;

  let mut codegen = ARM64Gen::new(&interner);
  let artifact = codegen.generate(&sir);

  // STR (immediate, unsigned offset, 64-bit): bits [31:22] =
  // 1111_1001_00. When the base register equals the source
  // register and imm12 is zero, the STR self-clobbers — the
  // address SP-add was emitted into the same register the
  // STR is consuming.
  for chunk in artifact.code.chunks_exact(4) {
    let insn = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);

    if (insn >> 22) != 0b1111100100 {
      continue;
    }

    let rt = insn & 0x1F;
    let rn = (insn >> 5) & 0x1F;
    let imm12 = (insn >> 10) & 0xFFF;

    assert!(
      rt != rn || imm12 != 0,
      "STR self-clobber: `str X{rt}, [X{rn}]` at offset 0",
    );
  }
}

// ================================================================
// `emit_add_sp_offset` must materialize the offset constant into
// `dst`, never into a hidden X16 temp. The earlier slow-path
// implementation always used X16 to load the offset before adding
// SP, which silently corrupted any caller-held value in X16 — for
// instance, a freshly-built tag word about to be spilled by
// `emit_str_sp`'s slow path. With `emit_str_sp(X16, big_off)`
// switching to X17 as the address scratch, the bug was a level
// deeper: `emit_add_sp_offset(X17, big_off)` still clobbered X16
// internally. Manifested at runtime as match-arm tags reading the
// numeric offset (e.g. 4280) instead of 0/1, so every match arm
// fell through and downstream operations like `HashMap::insert`
// silently no-oped.
// ================================================================

#[test]
fn test_emit_add_sp_offset_uses_dst_not_x16_in_slow_path() {
  // Force the slow path by piling enough `read_file` calls
  // (520 stack slots each, including the 4096-byte read
  // buffer) to push the next scratch base above 4095 — the
  // imm12 ceiling for the fast path. Each subsequent
  // `HashMap::insert` triggers `emit_add_sp_offset(X1, k_off)`
  // and `emit_add_sp_offset(X2, v_off)` at slow-path
  // offsets, which must materialize the constant into X1 / X2
  // (the dst) — never into X16 — so any caller-held X16 value
  // survives the address calculation.
  let mut interner = Interner::new();
  let main_sym = interner.intern("main");
  let read_sym = interner.intern("read_file");
  let map_new_sym = interner.intern("HashMap::new");
  let map_insert_sym = interner.intern("HashMap::insert");

  let mut sir = Sir::new();

  sir.emit(Insn::FunDef {
    name: main_sym,
    params: vec![],
    return_ty: TyId(1),
    body_start: 1,
    kind: FunctionKind::UserDefined,
    pubness: Pubness::No,
    mut_self: false,
  });

  let mut next: u32 = 0;
  let const_int = |sir: &mut Sir, next: &mut u32| -> ValueId {
    let v = ValueId(*next);
    *next += 1;
    sir.emit(Insn::ConstInt {
      dst: v,
      value: 0,
      ty_id: TyId(1),
    });
    v
  };
  let fresh = |next: &mut u32| -> ValueId {
    let v = ValueId(*next);
    *next += 1;
    v
  };

  let path_arg = const_int(&mut sir, &mut next);
  let read_dst = fresh(&mut next);

  sir.emit(Insn::Call {
    dst: read_dst,
    name: read_sym,
    args: vec![path_arg],
    ty_id: TyId(1),
  });

  let m_args = vec![
    const_int(&mut sir, &mut next),
    const_int(&mut sir, &mut next),
    const_int(&mut sir, &mut next),
  ];
  let m_handle = fresh(&mut next);

  sir.emit(Insn::Call {
    dst: m_handle,
    name: map_new_sym,
    args: m_args,
    ty_id: TyId(1),
  });

  // One insert at slow-path offsets exercises both
  // `emit_add_sp_offset(X1, k_off)` and `emit_add_sp_offset
  // (X2, v_off)` — both well past the imm12 fast path.
  let k = const_int(&mut sir, &mut next);
  let val = const_int(&mut sir, &mut next);
  let dst = fresh(&mut next);

  sir.emit(Insn::Call {
    dst,
    name: map_insert_sym,
    args: vec![m_handle, k, val],
    ty_id: TyId(1),
  });
  sir.emit(Insn::Return {
    value: None,
    ty_id: TyId(1),
  });

  sir.next_value_id = next;

  let mut codegen = ARM64Gen::new(&interner);
  let artifact = codegen.generate(&sir);

  // ADD (extended register, UXTX, shift=0): bits[31:21] =
  // 1000_1011_001 (`0x8B20_0000`), bits[15:13] = 011 (UXTX),
  // bits[12:10] = 000 (shift=0). With Rn = SP (31), this is
  // the slow-path `add Rd, SP, Rm` pattern. The fixed helper
  // emits `Rm == Rd` (materializer reuses the dst register);
  // the buggy helper emits `Rm == X16` regardless of `Rd`.
  let mut found_slow_path_add = false;

  for chunk in artifact.code.chunks_exact(4) {
    let insn = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);

    let is_add_ext = (insn & 0xFFE0_FC00) == 0x8B20_6000;

    if !is_add_ext {
      continue;
    }

    let rd = insn & 0x1F;
    let rn = (insn >> 5) & 0x1F;
    let rm = (insn >> 16) & 0x1F;

    // Skip prologue / epilogue frame adjustments
    // (`sub/add SP, SP, X16`) and the str-concat
    // dynamic-frame helper. Those legitimately use X16
    // because they predate any caller-held value in X16.
    if rn != 31 || rd == 31 {
      continue;
    }

    found_slow_path_add = true;

    assert!(
      rm == rd,
      "ADD X{rd}, SP, X{rm} — `emit_add_sp_offset` slow path \
       must materialize the offset into the dst register, not \
       into X16 (which may carry a caller-held value)",
    );
  }

  assert!(
    found_slow_path_add,
    "test setup did not exercise the slow path — no \
     `add Rd, SP, Rm` (ext-reg) in the emitted code; \
     bump the read_file count or struct-area pressure",
  );
}
