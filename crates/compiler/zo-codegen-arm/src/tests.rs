pub(crate) mod common;
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
use zo_value::ValueId;

#[test]
fn test_complete_pipeline_hello_world() {
  let source = r#"fun main() { show("hello world") }"#;

  let tokenizer = Tokenizer::new(source);
  let mut tokenization = tokenizer.tokenize();

  let parser = Parser::new(&tokenization, source);
  let parsing_result = parser.parse();

  let executor = Executor::new(
    &parsing_result.tree,
    &mut tokenization.interner,
    &tokenization.literals,
  );

  let (sir, _annotations, _ty_checker) = executor.execute();

  assert_eq!(sir.instructions.len(), 4);

  if let Insn::FunDef {
    name,
    params,
    body_start,
    ..
  } = &sir.instructions[0]
  {
    assert_eq!(tokenization.interner.get(*name), "main");
    assert_eq!(params.len(), 0);
    assert_eq!(*body_start, 1);
  } else {
    panic!("Expected FunDef instruction");
  }

  if let Insn::ConstString { symbol, .. } = &sir.instructions[1] {
    assert_eq!(tokenization.interner.get(*symbol), "hello world");
  } else {
    panic!("Expected ConstString instruction");
  }

  if let Insn::Call { name, args, .. } = &sir.instructions[2] {
    assert_eq!(tokenization.interner.get(*name), "show");
    assert_eq!(args.len(), 1);
  } else {
    panic!("Expected Call instruction");
  }

  assert!(matches!(
    sir.instructions[3],
    Insn::Return { value: None, .. }
  ));

  let mut codegen = ARM64Gen::new(&tokenization.interner);
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
  let tokenizer = Tokenizer::new(source);
  let mut tokenization = tokenizer.tokenize();

  let parser = Parser::new(&tokenization, source);
  let parsing = parser.parse();

  let executor = Executor::new(
    &parsing.tree,
    &mut tokenization.interner,
    &tokenization.literals,
  );

  let (sir, _, _) = executor.execute();

  let mut codegen = ARM64Gen::new(&tokenization.interner);
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
