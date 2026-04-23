//! Phase 5 of `PLAN_CHANNELS.md` — ARM codegen lowers the
//! concurrency SIR insns to `BL` placeholders against
//! the runtime symbol set:
//!
//! - `ChannelCreate` → `BL _zo_chan_new`
//! - `ChannelSend`   → `BL _zo_chan_send`
//! - `ChannelRecv`   → `BL _zo_chan_recv`
//! - `TaskSpawn`     → `BL _zo_task_spawn`
//! - `TaskAwait`     → `BL _zo_task_await`
//! - `NurseryBegin` / `NurseryEnd` — no code emitted
//!   (semantic markers only; cancellation wiring lives in
//!   the runtime).
//!
//! Phase 7 (not yet landed) wires `libzo_runtime.a` into
//! the Mach-O writer so these symbols resolve. Until then
//! these tests only check that the codegen pipeline records
//! the correct `extern_used` entries — confirming that
//! every concurrency insn passes through the runtime-call
//! path instead of being silently dropped by a wildcard
//! match arm.
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

  let (sir, _, _, _) = executor.execute();

  let mut codegen = ARM64Gen::new(&interner);
  let _artifact = codegen.generate(&sir);

  check(codegen.extern_used());
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
