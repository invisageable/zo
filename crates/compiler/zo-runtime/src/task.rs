//! Phase 6 of `PLAN_CHANNELS.md` — task runtime.
//!
//! One OS thread per `spawn`. The task's top-level frame
//! catches panics via `catch_unwind` and stashes the
//! outcome in the `ZoTask` handle; `_zo_task_await` pops
//! the outcome — `Completed` returns normally, `Panicked`
//! re-raises on the awaiting thread.
//!
//! This matches Phase 0 decision 1: no DWARF unwinding
//! across the runtime boundary, just a stored `Result`.
//!
//! ABI — the function pointer is `extern "C" fn()`. The
//! plan's `spawn callee(args)` syntax lowers to a thunk
//! capturing the args; Phase 5 codegen is responsible for
//! materializing that thunk. Phase 6 only defines the
//! void-arg entry point.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

/// Runtime state of a single spawned task.
pub struct ZoTask {
  /// OS thread handle. `Some` from spawn to await; `None`
  /// after the handle has been consumed.
  join: Mutex<Option<JoinHandle<()>>>,
  /// Terminal state, set by the task's own top-level
  /// `catch_unwind` frame. Populated before the thread
  /// exits, so it's always available by the time `join`
  /// returns.
  outcome: Arc<Mutex<TaskOutcome>>,
}

/// Task terminal states per Phase 0 decision 1.
#[derive(Debug)]
enum TaskOutcome {
  /// The task hasn't returned yet.
  Running,
  /// Normal return from the callee. Phase 6 MVP doesn't
  /// carry the return value through this path — a richer
  /// `Completed(Vec<u8>)` lands when the nursery-await
  /// wiring is exercised end-to-end.
  Completed,
  /// The task panicked. Payload discarded for MVP; only
  /// "a panic occurred" is preserved. Richer payload
  /// carry-through is a follow-up.
  Panicked,
}

/// Spawn a C-ABI void callable on a fresh OS thread.
///
/// The `callee` uses the `C-unwind` ABI so a panic
/// originating inside a zo task can legally unwind out
/// through the function-pointer boundary; without it,
/// Rust's default `extern "C"` treats such unwinding as
/// UB and aborts the process. The catch_unwind inside
/// the worker thread converts the unwind into a stored
/// `Panicked` outcome per Phase 0 decision 1.
///
/// # Safety
///
/// `callee` must be a live function pointer whose body is
/// safe to call with zero arguments. Typical caller — the
/// ARM codegen emits a stub that lowers `spawn f(a, b)`
/// to a captured-arg thunk matching this signature.
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn _zo_task_spawn(
  callee: extern "C-unwind" fn(),
) -> *mut ZoTask {
  let outcome = Arc::new(Mutex::new(TaskOutcome::Running));
  let worker_outcome = outcome.clone();

  let join = thread::spawn(move || {
    // AssertUnwindSafe: `callee` is `extern "C" fn()` —
    // no captures, no shared mutable state that could
    // be invalidated by a panic.
    let result = catch_unwind(AssertUnwindSafe(|| callee()));

    let mut slot = worker_outcome.lock().expect("zo-task outcome poisoned");

    *slot = match result {
      Ok(()) => TaskOutcome::Completed,
      Err(_) => TaskOutcome::Panicked,
    };
  });

  Box::into_raw(Box::new(ZoTask {
    join: Mutex::new(Some(join)),
    outcome,
  }))
}

/// Block until `task` finishes, then release the handle.
/// Panics on the awaiting thread if the task itself
/// panicked — Phase 0 decision 1's propagation semantics.
///
/// # Safety
///
/// `task` must have come from [`_zo_task_spawn`] and must
/// not be referenced again after this call returns.
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn _zo_task_await(task: *mut ZoTask) {
  if task.is_null() {
    return;
  }

  // SAFETY: caller contract — exclusive ownership.
  let handle = unsafe { Box::from_raw(task) };

  if let Some(join) = handle.join.lock().expect("zo-task join poisoned").take()
    && let Err(payload) = join.join()
  {
    // The catch_unwind above should have already set the
    // outcome to Panicked — but `thread::spawn`'s closure
    // itself could panic before `catch_unwind` runs (e.g.
    // an OOM before the call). Re-raise in that case to
    // keep the propagation invariant.
    std::panic::resume_unwind(payload);
  }

  let outcome = std::mem::replace(
    &mut *handle.outcome.lock().expect("zo-task outcome poisoned"),
    TaskOutcome::Completed,
  );

  if matches!(outcome, TaskOutcome::Panicked) {
    panic!("zo-task panicked — propagating to awaiting thread");
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::sync::atomic::{AtomicU32, Ordering};

  static COUNTER: AtomicU32 = AtomicU32::new(0);

  extern "C-unwind" fn increment_counter() {
    COUNTER.fetch_add(1, Ordering::SeqCst);
  }

  extern "C-unwind" fn panicking_task() {
    panic!("intentional");
  }

  #[test]
  fn spawn_await_runs_the_callee_once() {
    COUNTER.store(0, Ordering::SeqCst);

    unsafe {
      let task = _zo_task_spawn(increment_counter);

      _zo_task_await(task);
    }

    assert_eq!(COUNTER.load(Ordering::SeqCst), 1);
  }

  #[test]
  #[should_panic(expected = "zo-task panicked")]
  fn await_on_panicked_task_propagates() {
    unsafe {
      let task = _zo_task_spawn(panicking_task);

      _zo_task_await(task);
    }
  }
}
