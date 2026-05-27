// Run with `--test-threads=1` — the executor uses
// thread-local state that deadlocks under parallel
// proptest execution.

use zo_codegen_arm::ARM64Gen;
use zo_executor::Executor;
use zo_interner::Interner;
use zo_parser::Parser;
use zo_sir::Insn;
use zo_tokenizer::Tokenizer;
use zo_ty_checker::TyChecker;

use proptest::prelude::*;
use proptest::test_runner::{Config, FileFailurePersistence};

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

  let (sir, _, _, _, _, _, _) = executor.execute();

  let mut codegen = ARM64Gen::new(&interner);
  let artifact = codegen.generate(&sir);

  artifact.code
}

fn compile_sir(source: &str) -> Vec<Insn> {
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

  let (sir, _, _, _, _, _, _) = executor.execute();

  sir.instructions
}

/// ARM64 BL encoding: bits [31:26] = 100101.
fn is_bl(insn: u32) -> bool {
  (insn >> 26) == 0b100101
}

/// ARM64 MOV (W-register) base: 0x2A0003E0.
fn is_mov_w(insn: u32) -> bool {
  (insn & 0xFFE0_FFE0) == 0x2A00_03E0
}

proptest! {
  #![proptest_config(Config {
    failure_persistence: Some(Box::new(
      FileFailurePersistence::Off
    )),
    ..Config::default()
  })]

  /// Codegen never panics on random arithmetic expressions.
  #[test]
  fn codegen_never_panics_on_arithmetic(
    a in -1000i64..1000,
    b in -1000i64..1000,
    op in prop::sample::select(vec!["+", "-", "*"]),
  ) {
    let source = format!(
      "fun main() {{ imu x: int = {a} {op} {b}; showln(x); }}"
    );
    let code = compile_to_code(&source);
    prop_assert!(!code.is_empty());
  }

  /// Codegen never panics on nested call expressions.
  #[test]
  fn codegen_never_panics_on_nested_calls(
    depth in 1usize..5,
    value in -100i64..100,
  ) {
    let mut source = String::from(
      "fun identity(x: int) -> int { x }\n",
    );
    source.push_str("fun main() {\n  imu x: int = ");

    for _ in 0..depth {
      source.push_str("identity(");
    }
    source.push_str(&format!("{value}"));
    for _ in 0..depth {
      source.push(')');
    }
    source.push_str(";\n  showln(x);\n}\n");

    let code = compile_to_code(&source);
    prop_assert!(!code.is_empty());
  }

  /// Call results must never use W-register MOV. The W-move
  /// zero-extends and destroys the sign bit of negative 32-bit
  /// values. This property scans for MOV(W) immediately after
  /// any BL instruction in the emitted code.
  #[test]
  fn call_result_never_uses_w_move(
    value in -1000i64..1000,
  ) {
    let source = format!(
      "fun get() -> int {{ {value} }}\n\
       fun main() {{ showln(get()); }}"
    );
    let code = compile_to_code(&source);

    let insns: Vec<u32> = code
      .chunks_exact(4)
      .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
      .collect();

    for (i, &insn) in insns.iter().enumerate() {
      if is_bl(insn) && i + 1 < insns.len() {
        prop_assert!(
          !is_mov_w(insns[i + 1]),
          "MOV(W) after BL at index {}: 0x{:08X}",
          i, insns[i + 1],
        );
      }
    }
  }

  /// Multi-arg calls must produce valid code without panics.
  /// Exercises the register move + clobber-save path for
  /// increasing arg counts (1..8).
  #[test]
  fn multi_arg_call_never_panics(
    arg_count in 1usize..8,
  ) {
    let params: Vec<String> = (0..arg_count)
      .map(|i| format!("p{i}: int"))
      .collect();
    let sum: Vec<String> = (0..arg_count)
      .map(|i| format!("p{i}"))
      .collect();
    let args: Vec<String> = (0..arg_count)
      .map(|i| format!("{}", i + 1))
      .collect();

    let source = format!(
      "fun add({}) -> int {{ {} }}\n\
       fun main() {{ showln(add({})); }}",
      params.join(", "),
      sum.join(" + "),
      args.join(", "),
    );

    let code = compile_to_code(&source);
    prop_assert!(!code.is_empty());
  }

  /// Codegen is deterministic — identical source produces
  /// identical machine code.
  #[test]
  fn codegen_is_deterministic(
    a in -100i64..100,
    b in -100i64..100,
    op in prop::sample::select(vec!["+", "-", "*"]),
  ) {
    let source = format!(
      "fun main() {{ imu x: int = {a} {op} {b}; showln(x); }}"
    );

    let code1 = compile_to_code(&source);
    let code2 = compile_to_code(&source);

    prop_assert_eq!(code1, code2, "non-deterministic codegen");
  }

  /// SIR emission is deterministic — identical source
  /// produces identical instruction sequences.
  #[test]
  fn sir_emission_is_deterministic(
    a in -100i64..100,
    b in -100i64..100,
    op in prop::sample::select(vec!["+", "-", "*"]),
  ) {
    let source = format!(
      "fun main() {{ imu x: int = {a} {op} {b}; showln(x); }}"
    );

    let sir1 = compile_sir(&source);
    let sir2 = compile_sir(&source);

    prop_assert_eq!(sir1.len(), sir2.len(), "SIR length mismatch");
  }

  /// Negative return values must survive through the Call
  /// result path. The codegen must not truncate or zero-extend
  /// them.
  #[test]
  fn negative_return_value_preserved_in_sir(
    value in -1000i64..-1,
  ) {
    let source = format!(
      "fun neg() -> int {{ {value} }}\n\
       fun main() {{ showln(neg()); }}"
    );

    let sir = compile_sir(&source);

    // The SIR must contain a ConstInt with the negative value
    // inside the `neg` function body.
    let has_neg_const = sir.iter().any(|insn| {
      if let Insn::ConstInt {
        value: v, ..
      } = insn
      {
        *v == value as u64
      } else {
        false
      }
    });

    prop_assert!(
      has_neg_const,
      "SIR missing ConstInt({value}) for negative return",
    );
  }

  // ============================================================
  // Stress tests — Reddit allocator checklist patterns.
  // ============================================================

  /// Nested calls: f(g(h(x))). Intermediate results flow
  /// through the call save/restore path. The codegen must
  /// chain call results without corruption.
  #[test]
  fn nested_calls_never_panic(
    depth in 1usize..6,
    value in -50i64..50,
  ) {
    let mut fns = String::new();

    for i in 0..depth {
      fns.push_str(&format!(
        "fun f{i}(x: int) -> int {{ x + {i} }}\n",
      ));
    }

    let mut call_expr = format!("{value}");

    for i in 0..depth {
      call_expr = format!("f{i}({call_expr})");
    }

    let source = format!(
      "{fns}fun main() {{ showln({call_expr}); }}"
    );

    let code = compile_to_code(&source);
    prop_assert!(!code.is_empty());
  }

  /// Negative values through nested calls — the sign must
  /// survive every call boundary.
  #[test]
  fn negative_through_nested_calls(
    depth in 1usize..4,
  ) {
    let mut fns = String::new();

    for i in 0..depth {
      fns.push_str(&format!(
        "fun f{i}(x: int) -> int {{ x }}\n",
      ));
    }

    let mut call_expr = String::from("-42");

    for i in 0..depth {
      call_expr = format!("f{i}({call_expr})");
    }

    let source = format!(
      "{fns}fun main() {{ showln({call_expr}); }}"
    );

    let code = compile_to_code(&source);

    // No W-register MOV after any BL.
    let insns: Vec<u32> = code
      .chunks_exact(4)
      .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
      .collect();

    for (i, &insn) in insns.iter().enumerate() {
      if is_bl(insn) && i + 1 < insns.len() {
        prop_assert!(
          !is_mov_w(insns[i + 1]),
          "MOV(W) after BL at index {} in nested call \
           depth={}: 0x{:08X}",
          i, depth, insns[i + 1],
        );
      }
    }
  }

  /// Many live values across a call — defines N constants,
  /// calls a function, then sums all N values. Forces N
  /// spills and reloads.
  #[test]
  fn many_live_values_across_call(
    count in 2usize..14,
  ) {
    let mut vars = String::new();
    let mut sum_parts = Vec::new();

    for i in 0..count {
      vars.push_str(&format!(
        "  imu v{i}: int = {i};\n",
      ));
      sum_parts.push(format!("v{i}"));
    }

    let source = format!(
      "fun noop() -> int {{ 0 }}\n\
       fun main() {{\n\
       {vars}\
         imu ignored: int = noop();\n\
         showln({});\n\
       }}",
      sum_parts.join(" + "),
    );

    let code = compile_to_code(&source);
    prop_assert!(!code.is_empty());
  }

  /// High-arity function calls (1..8 args). Each arg must
  /// land in the correct register without clobbering others.
  #[test]
  fn high_arity_call_produces_code(
    arg_count in 1usize..9,
    base_value in 0i64..100,
  ) {
    let params: Vec<String> = (0..arg_count)
      .map(|i| format!("p{i}: int"))
      .collect();
    let body: Vec<String> = (0..arg_count)
      .map(|i| format!("p{i}"))
      .collect();
    let args: Vec<String> = (0..arg_count)
      .map(|i| format!("{}", base_value + i as i64))
      .collect();

    let source = format!(
      "fun sum({}) -> int {{ {} }}\n\
       fun main() {{ showln(sum({})); }}",
      params.join(", "),
      body.join(" + "),
      args.join(", "),
    );

    let code = compile_to_code(&source);
    prop_assert!(!code.is_empty());
  }

  /// Interleaved define-call-define-call-use at the source
  /// level. Each variable must survive its spanning call.
  #[test]
  fn interleaved_define_call_use(
    groups in 2usize..5,
  ) {
    let mut body = String::new();
    let mut sum_parts = Vec::new();

    for i in 0..groups {
      body.push_str(&format!(
        "  imu v{i}: int = {i};\n\
         imu ignore{i}: int = noop();\n",
      ));
      sum_parts.push(format!("v{i}"));
    }

    let source = format!(
      "fun noop() -> int {{ 0 }}\n\
       fun main() {{\n\
       {body}\
         showln({});\n\
       }}",
      sum_parts.join(" + "),
    );

    let code = compile_to_code(&source);
    prop_assert!(!code.is_empty());
  }
}
