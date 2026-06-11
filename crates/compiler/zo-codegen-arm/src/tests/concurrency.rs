//! ARM codegen lowers the concurrency SIR insns to
//! `BL` placeholders against the runtime symbol set:
//!
//! - `ChannelCreate` → `BL _zo_chan_new`
//! - `ChannelSend`   → `BL _zo_chan_send`
//! - `ChannelRecv`   → `BL _zo_chan_recv`
//! - `TaskSpawn`     → `BL _zo_task_spawn`
//! - `TaskAwait`     → `BL _zo_task_await`
//! - `NurseryBegin` / `NurseryEnd` — no code emitted
//!   (semantic markers only; cancellation wiring lives
//!   in the runtime).
//!
//! These tests check that the codegen pipeline records
//! the correct `extern_used` entries — confirming that
//! every concurrency insn passes through the runtime-
//! call path instead of being silently dropped by a
//! wildcard match arm.
//!
//! ```sh
//! cargo test -p zo-codegen-arm concurrency
//! ```

use crate::ARM64Gen;

use zo_executor::Executor;
use zo_interner::Interner;
use zo_parser::Parser;
use zo_tokenizer::Tokenizer;
use zo_ty_checker::TyChecker;

fn compile_and_inspect<F: FnOnce(&[String])>(source: &str, check: F) {
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

  let sir = executor.execute().sir;

  let mut codegen = ARM64Gen::new(&interner);
  let _artifact = codegen.generate(&sir);

  check(codegen.extern_used());
}

/// Compiles `source` to a full Mach-O binary (via
/// `generate_macho`) and hands the raw bytes to
/// `check`. Used to confirm the binary carries the
/// right `LC_LOAD_DYLIB` entries for programs that
/// use concurrency.
fn compile_macho_and_inspect<F: FnOnce(&[u8])>(source: &str, check: F) {
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

  let sir = executor.execute().sir;

  let mut codegen = ARM64Gen::new(&interner);
  let artifact = codegen.generate(&sir);
  let link_obj = codegen.into_link_object(artifact);
  let binary = zo_linker::link_macho(
    link_obj,
    zo_codegen_backend::Target::Arm64AppleDarwin,
  )
  .executable;

  check(&binary);
}

/// Compiles `source` to a full `LinkOutput` (executable
/// bytes + the runtime flavor the linker selected) and
/// hands it to `check`. Exercises the real resolution path
/// — SIR fragment → `MachoLinkObject` → `link_macho` — so
/// the runtime-kind verdict is the genuine one the compiler
/// stages from.
fn compile_link_output_and_inspect<F: FnOnce(&zo_linker::LinkOutput)>(
  source: &str,
  check: F,
) {
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

  let sir = executor.execute().sir;

  let mut codegen = ARM64Gen::new(&interner);
  let artifact = codegen.generate(&sir);
  let link_obj = codegen.into_link_object(artifact);

  check(&zo_linker::link_macho(
    link_obj,
    zo_codegen_backend::Target::Arm64AppleDarwin,
  ));
}

fn contains_bytes(binary: &[u8], needle: &[u8]) -> bool {
  binary.windows(needle.len()).any(|w| w == needle)
}

#[test]
fn channel_create_registers_chan_new_extern() {
  compile_and_inspect(
    r#"
      fun main() {
        nursery {
          imu (tx, rx) := channel();
        }
      }
    "#,
    |externs| {
      assert!(
        externs.iter().any(|s| s == "_zo_chan_new"),
        "expected `_zo_chan_new` in extern_used, got {externs:?}"
      );
    },
  );
}

#[test]
fn task_spawn_registers_task_spawn_extern() {
  compile_and_inspect(
    r#"
      fun worker() {}
      fun main() {
        nursery {
          spawn worker();
        }
      }
    "#,
    |externs| {
      assert!(
        externs.iter().any(|s| s == "_zo_task_spawn"),
        "expected `_zo_task_spawn` in extern_used, got {externs:?}"
      );
    },
  );
}

#[test]
fn concurrency_binary_loads_runtime_dylib() {
  // A program using `channel()` must emit an
  // `LC_LOAD_DYLIB` pointing at libzo_runtime.dylib so
  // dyld can resolve `_zo_chan_new` at load time. The
  // path is `@loader_path/deps/...` — the compiler's
  // `deps/` invariant pairs with `stage_runtime_artifacts`
  // copying the dylib into `<binary_dir>/deps/`.
  compile_macho_and_inspect(
    r#"
      fun main() {
        nursery {
          imu (tx, rx) := channel();
        }
      }
    "#,
    |binary| {
      assert!(
        contains_bytes(binary, b"@loader_path/deps/libzo_runtime.dylib"),
        "expected runtime dylib LC_LOAD_DYLIB string in the binary"
      );
      // libSystem still needed for intrinsics + syscalls.
      assert!(
        contains_bytes(binary, b"/usr/lib/libSystem.B.dylib"),
        "libSystem LC_LOAD_DYLIB must still be present"
      );
    },
  );
}

#[test]
fn non_concurrency_binary_omits_runtime_dylib() {
  // A program that never touches a concurrency insn
  // must NOT pull in libzo_runtime.dylib — registering a
  // dylib the binary doesn't reference bloats load-time
  // bookkeeping.
  compile_macho_and_inspect(
    r#"
      fun main() {
        imu x: int = 42;
      }
    "#,
    |binary| {
      assert!(
        !contains_bytes(binary, b"libzo_runtime.dylib"),
        "expected no runtime dylib entry for a non-concurrency program"
      );
      assert!(
        contains_bytes(binary, b"/usr/lib/libSystem.B.dylib"),
        "libSystem LC_LOAD_DYLIB must be present"
      );
    },
  );
}

#[test]
fn nursery_markers_do_not_emit_externs() {
  // NurseryBegin / NurseryEnd lower to no code — they're
  // semantic markers. Stand-alone, no concurrency insns
  // beyond the nursery scope itself should trigger any
  // runtime-call entries.
  compile_and_inspect(
    r#"
      fun main() {
        nursery {}
      }
    "#,
    |externs| {
      // libc `malloc` / `free` may show up via other
      // paths depending on the test build — assert only
      // that none of the concurrency runtime symbols fire.
      for sym in externs {
        assert!(
          !sym.starts_with("_zo_chan_") && !sym.starts_with("_zo_task_"),
          "bare nursery should not pull in a concurrency runtime symbol, got {sym}"
        );
      }
    },
  );
}

#[test]
fn concurrency_program_selects_lean_runtime() {
  // Channel / task symbols all live in the lean core, so a
  // pure-concurrency program must stage the lean dylib.
  compile_link_output_and_inspect(
    r#"
      fun main() {
        nursery {
          imu (tx, rx) := channel();
        }
      }
    "#,
    |output| {
      assert_eq!(
        output.runtime,
        zo_linker::RuntimeKind::Lean,
        "concurrency-only program must select the lean runtime"
      );
    },
  );
}

#[test]
fn non_runtime_program_selects_no_runtime() {
  // No `_zo_*` import at all — nothing to stage.
  compile_link_output_and_inspect(
    r#"
      fun main() {
        imu x: int = 42;
      }
    "#,
    |output| {
      assert_eq!(
        output.runtime,
        zo_linker::RuntimeKind::None,
        "program with no runtime symbol must select RuntimeKind::None"
      );
    },
  );
}
