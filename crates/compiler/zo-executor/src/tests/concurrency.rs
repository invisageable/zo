//! Executor support for `nursery { }`, `spawn`,
//! `await`, `channel()`, and the `tx.send` / `rx.recv`
//! method pair.
//!
//! Tests assert SIR shape via structural predicates rather
//! than exact equality, since surrounding insns
//! (`FunDef`, `Load`, etc.) are incidental and would break
//! the contract on unrelated executor tweaks.
//!
//! ```sh
//! cargo test -p zo-executor concurrency
//! ```

use crate::tests::common::{assert_execution_error, assert_sir_structure};

use zo_error::ErrorKind;
use zo_sir::Insn;

fn count_matching<P: Fn(&Insn) -> bool>(insns: &[Insn], pred: P) -> usize {
  insns.iter().filter(|i| pred(i)).count()
}

#[test]
fn nursery_empty_emits_paired_begin_end() {
  assert_sir_structure(
    r#"
      fun main() {
        nursery {}
      }
    "#,
    |insns| {
      let begins =
        count_matching(insns, |i| matches!(i, Insn::NurseryBegin { .. }));
      let ends =
        count_matching(insns, |i| matches!(i, Insn::NurseryEnd { .. }));

      assert_eq!(begins, 1, "expected one NurseryBegin, got {insns:#?}");
      assert_eq!(ends, 1, "expected one NurseryEnd, got {insns:#?}");
    },
  );
}

#[test]
fn channel_builtin_emits_channel_create_and_tuple() {
  assert_sir_structure(
    r#"
      fun main() {
        nursery {
          imu (tx, rx) := channel();
        }
      }
    "#,
    |insns| {
      let created =
        count_matching(insns, |i| matches!(i, Insn::ChannelCreate { .. }));
      let tupled =
        count_matching(insns, |i| matches!(i, Insn::TupleLiteral { .. }));

      assert_eq!(created, 1, "expected ChannelCreate, got {insns:#?}");
      assert!(
        tupled >= 1,
        "expected TupleLiteral from channel(), got {insns:#?}"
      );
    },
  );
}

#[test]
fn channel_with_explicit_literal_capacity() {
  assert_sir_structure(
    r#"
      fun main() {
        nursery {
          imu (tx, rx) := channel(4);
        }
      }
    "#,
    |insns| {
      let create = insns
        .iter()
        .find(|i| matches!(i, Insn::ChannelCreate { .. }))
        .expect("ChannelCreate missing");

      if let Insn::ChannelCreate { capacity, .. } = create {
        assert_eq!(*capacity, 4, "capacity should be 4, got {capacity}");
      }
    },
  );
}

// KNOWN GAP — surface tuple destructure vs Channel types:
//
// `imu (tx, rx) := channel();` goes through the imu
// tuple-destructure finalize path, which extracts each
// binding via `TupleIndex(tuple_sir, i)`. That path
// currently propagates `TyId::unit` to each binding
// rather than the tuple's element type (ChannelTx /
// ChannelRx), so subsequent `tx.send(..)` / `rx.recv()`
// calls see a Unit receiver and don't match the
// channel guard in `execute_potential_call`.
//
// The send / recv dispatch arms ARE wired correctly
// and emit `ChannelSend` / `ChannelRecv` when the
// receiver type resolves to a channel ty. They just
// aren't exercisable from a pure-source program until
// the destructure types flow through properly.

#[test]
fn spawn_outside_nursery_is_error() {
  // No implicit main-nursery. A bare `spawn f()` at
  // main level is a hard error.
  assert_execution_error(
    r#"
      fun worker() {}
      fun main() {
        spawn worker();
      }
    "#,
    ErrorKind::SpawnOutsideNursery,
  );
}

#[test]
fn spawn_inside_nursery_emits_task_spawn_not_call() {
  // `spawn worker()` inside a `nursery { }` must redirect
  // the would-be `Call` emission into a `TaskSpawn`. The
  // fn-local `Call` (from inside `worker`'s own body) would
  // still fire for any statements there, but for this
  // unit-body worker the only `Call`-ish emission is the
  // spawn itself.
  assert_sir_structure(
    r#"
      fun worker() {}
      fun main() {
        nursery {
          spawn worker();
        }
      }
    "#,
    |insns| {
      let spawns =
        count_matching(insns, |i| matches!(i, Insn::TaskSpawn { .. }));
      let regular_calls =
        count_matching(insns, |i| matches!(i, Insn::Call { .. }));

      assert_eq!(spawns, 1, "expected one TaskSpawn, got {insns:#?}");
      assert_eq!(
        regular_calls, 0,
        "no regular Call should be emitted for the spawned worker, got {insns:#?}"
      );
    },
  );
}

#[test]
fn channel_capacity_variable_is_error() {
  // Capacity must be an integer literal. A variable
  // reference is a hard error even if the value would
  // have been a valid u32.
  assert_execution_error(
    r#"
      fun main() {
        val BUF: int = 4;
        nursery {
          imu (tx, rx) := channel(BUF);
        }
      }
    "#,
    ErrorKind::ChannelCapacityNotLiteral,
  );
}
