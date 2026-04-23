//! Phase 3 of `PLAN_PREHISTORY.md` — green task
//! lifecycle.
//!
//! A `ZoTask` owns the state that makes a green task
//! schedulable: a saved `Context`, a dedicated stack, a
//! state machine (`Ready → Running → Blocked|Dead`), a
//! terminal outcome (`Completed | Panicked`), and a
//! waiter list for `await` propagation.
//!
//! `ZoTask` operates hand-in-hand with `scheduler.rs`,
//! which owns the run queue and the yield / drain
//! primitives. This module owns task identity and
//! lifecycle; the scheduler module owns scheduling
//! policy. The boundary:
//!
//! - `task.rs` — `ZoTask`, `task_shim`, `exit_current`,
//!   `_zo_task_spawn` / `_zo_task_await` ABI exports.
//! - `scheduler.rs` — `yield_now`, `run_one`,
//!   `drain_until_dead`, thread-local queue state.
//!
//! ABI stability — `_zo_task_spawn` and `_zo_task_await`
//! keep the signatures they had under PLAN_CHANNELS
//! Phase 6's pthread-based runtime. ARM codegen emits
//! BL placeholders that resolve to these symbols; the
//! swap from pthread to green threads is transparent to
//! the compiler.

use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::ctxsw::{Context, ctx_switch};
use crate::scheduler;

/// Per-task stack size. D2 of PLAN_PREHISTORY pins this
/// at 256 KB. Phase 3 MVP uses a plain `Box<[u8]>` —
/// no guard page, no VM reservation. Phase 8 swaps to
/// `mmap` + `mprotect` so stack overflow becomes a
/// clean segfault instead of heap corruption.
const DEFAULT_STACK_SIZE: usize = 256 * 1024;

/// Lifecycle states a task moves through. The state
/// machine is driven entirely by the scheduler thread
/// (v1: single OS thread per scheduler).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskState {
  /// On the run queue, waiting for a turn.
  Ready,
  /// Currently executing on the scheduler's OS thread.
  Running,
  /// Parked on a channel wait queue, task handle, or
  /// similar — not on the run queue. Wake-up
  /// transitions back to `Ready`.
  Blocked,
  /// Task body has returned (normally or via panic).
  /// The task struct lives until its waiters and/or
  /// the explicit `_zo_task_await` consume it.
  Dead,
}

/// Terminal outcome of a task body.
///
/// Per PLAN_CHANNELS Phase 0 decision 1, we store the
/// outcome instead of unwinding across the FFI
/// boundary. `await` re-raises if `Panicked`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskOutcome {
  /// Body hasn't returned yet.
  Running,
  /// Normal return. Phase 3 MVP doesn't thread the
  /// value back — a richer `Completed(Vec<u8>)` lands
  /// when the type-aware return path is wired.
  Completed,
  /// Body panicked. Payload is discarded in v1;
  /// Phase 7+ can carry it.
  Panicked,
}

/// A green task — the unit the scheduler schedules.
///
/// All fields are accessed only from the scheduler OS
/// thread (v1). Raw `*mut ZoTask` pointers are the
/// handle type that crosses ABI boundaries.
pub struct ZoTask {
  /// Saved CPU state — restored on every resume, saved
  /// on every yield.
  pub ctx: Context,
  /// Where the task is in its lifecycle.
  pub state: TaskState,
  /// How the task ended, once `state == Dead`.
  pub outcome: TaskOutcome,
  /// Task-owned stack. Kept alive for the lifetime of
  /// the task struct.
  _stack: Box<[u8]>,
  /// User callee address, passed via `x20` to the task
  /// shim on first enter. Stored to survive the Box's
  /// move during construction.
  user_entry_addr: u64,
  /// Tasks that have parked on `await`-ing this one.
  /// On transition to `Dead`, each is marked `Ready`
  /// and pushed back to the run queue.
  pub waiters: Vec<*mut ZoTask>,
}

impl ZoTask {
  /// Allocate a new task. The task is `Ready` and its
  /// Context bootstraps into [`task_shim`] on the first
  /// `ctx_switch` into it.
  fn new(user_entry: extern "C-unwind" fn()) -> Box<Self> {
    let mut stack = vec![0u8; DEFAULT_STACK_SIZE].into_boxed_slice();
    let stack_top = unsafe { stack.as_mut_ptr().add(stack.len()) };

    let mut task = Box::new(Self {
      ctx: Context::zeroed(),
      state: TaskState::Ready,
      outcome: TaskOutcome::Running,
      _stack: stack,
      user_entry_addr: user_entry as *const () as u64,
      waiters: Vec::new(),
    });

    // Carry the task's own address through to the
    // shim, so the shim can read `user_entry_addr` and
    // flip `state` / `outcome` at the end.
    let task_addr = &mut *task as *mut ZoTask as u64;

    task.ctx.bootstrap(stack_top, task_shim, task_addr);

    task
  }
}

/// Runs on the task's own stack. Invokes the user
/// callee inside `catch_unwind`, records the outcome,
/// and hands control back to the scheduler via
/// [`exit_current`].
extern "C-unwind" fn task_shim(task_addr: u64) {
  // SAFETY: `task_addr` was set by `ZoTask::new` and
  // the task's `Box<ZoTask>` is still live — the
  // scheduler drops it only after the `Dead`
  // transition is consumed by `await_task`.
  let task = task_addr as *mut ZoTask;
  let user_entry_addr = unsafe { (*task).user_entry_addr };

  // SAFETY: `user_entry_addr` was constructed from a
  // valid `extern "C-unwind" fn()` pointer.
  let user_entry: extern "C-unwind" fn() = unsafe {
    std::mem::transmute::<*const (), extern "C-unwind" fn()>(
      user_entry_addr as *const (),
    )
  };

  // `catch_unwind` needs `FnOnce()`; wrap the extern
  // fn in a closure. Zero captures — vanishes after
  // optimization.
  let result = catch_unwind(AssertUnwindSafe(|| user_entry()));

  // SAFETY: same task pointer as above, still live.
  unsafe {
    (*task).outcome = match result {
      Ok(()) => TaskOutcome::Completed,
      Err(_) => TaskOutcome::Panicked,
    };
  }

  exit_current();
}

/// Terminates the current task. Sets `state = Dead`,
/// marks every waiter `Ready`, pushes them to the run
/// queue, then `ctx_switch`es back to the scheduler.
/// Never returns.
fn exit_current() -> ! {
  scheduler::with(|s| {
    let task = s
      .current()
      .expect("exit_current called outside a task context");

    // SAFETY: task pointer is live (the Box is owned
    // by the awaiter or by the scheduler); we hold
    // exclusive access while `Running`.
    unsafe {
      (*task).state = TaskState::Dead;

      let waiters = std::mem::take(&mut (*task).waiters);

      for w in waiters {
        (*w).state = TaskState::Ready;
        s.enqueue(w);
      }

      ctx_switch(&raw mut (*task).ctx, s.scheduler_ctx_ptr());
    }
  });

  // SAFETY: the ctx_switch above transferred to the
  // scheduler and won't return — the task is `Dead`
  // and the scheduler won't re-enter it.
  unsafe { std::hint::unreachable_unchecked() }
}

/// Spawn a new green task. Allocates the ZoTask and
/// pushes it to the scheduler's run queue.
///
/// # Safety
///
/// `user_entry` must be a live function pointer; the
/// returned handle is consumed by [`await_task`] or
/// equivalent.
pub unsafe fn spawn(user_entry: extern "C-unwind" fn()) -> *mut ZoTask {
  let task = Box::into_raw(ZoTask::new(user_entry));

  scheduler::with(|s| s.enqueue(task));

  task
}

/// Await `target`. If called from a task, parks the
/// caller until the target dies; if called from the
/// non-task scheduler thread (main inside a nursery),
/// drains the run queue. Re-raises on the awaiter if
/// the target panicked.
///
/// # Safety
///
/// `target` must be a `*mut ZoTask` produced by
/// [`spawn`] (or the C ABI equivalent) and not yet
/// freed.
pub unsafe fn await_task(target: *mut ZoTask) {
  let caller = scheduler::with(|s| s.current());

  match caller {
    Some(current) => {
      // Task-awaiting-task — park + yield.
      // SAFETY: caller contract.
      if unsafe { (*target).state } != TaskState::Dead {
        unsafe {
          (*target).waiters.push(current);
          (*current).state = TaskState::Blocked;
        }

        unsafe { scheduler::yield_now() };

        // Resumed via `exit_current`'s waiter flush.
      }
    }
    None => {
      // Non-task context (main). Drain the scheduler.
      unsafe { scheduler::drain_until_dead(target) };
    }
  }

  // Consume the outcome and free the task struct.
  // SAFETY: exclusive ownership transfers to this Box;
  // no one else references `target` after confirming
  // it's Dead.
  let task = unsafe { Box::from_raw(target) };

  if matches!(task.outcome, TaskOutcome::Panicked) {
    panic!("zo-task panicked — propagating to awaiter");
  }
}

// ===== C ABI exports =====
//
// Same symbol names and signatures as PLAN_CHANNELS
// Phase 6's pthread-based runtime — ARM codegen emits
// BL placeholders resolving to these.

/// Spawn a new green task. Returns an opaque handle
/// consumed by `_zo_task_await`.
///
/// # Safety
///
/// `callee` must be a live `extern "C-unwind"` function
/// pointer. The returned handle remains valid until
/// consumed by `_zo_task_await` — dropping it without
/// awaiting leaks the task's stack and ZoTask struct.
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn _zo_task_spawn(
  callee: extern "C-unwind" fn(),
) -> *mut ZoTask {
  unsafe { spawn(callee) }
}

/// Await a task. Blocks the caller until the task
/// completes. Re-raises a task panic on the caller.
///
/// # Safety
///
/// `task` must have come from `_zo_task_spawn` and not
/// yet been awaited. After this call returns, `task`
/// is freed and must not be referenced again.
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn _zo_task_await(task: *mut ZoTask) {
  if task.is_null() {
    return;
  }

  unsafe { await_task(task) };
}

// ===== Tests =====

#[cfg(test)]
mod tests {
  use super::*;

  use std::sync::atomic::{AtomicU32, Ordering};

  static COUNTER: AtomicU32 = AtomicU32::new(0);

  extern "C-unwind" fn increment_counter() {
    COUNTER.fetch_add(1, Ordering::SeqCst);
  }

  extern "C-unwind" fn yield_then_increment() {
    unsafe { scheduler::yield_now() };
    COUNTER.fetch_add(1, Ordering::SeqCst);
  }

  extern "C-unwind" fn panicking_task() {
    panic!("intentional green-task panic");
  }

  #[test]
  fn spawn_await_runs_entry_once() {
    scheduler::reset_for_test();

    COUNTER.store(0, Ordering::SeqCst);

    unsafe {
      let task = _zo_task_spawn(increment_counter);

      _zo_task_await(task);
    }

    assert_eq!(COUNTER.load(Ordering::SeqCst), 1);
  }

  #[test]
  fn spawn_many_tasks_all_run() {
    // Fan-out a small batch — proves the run queue
    // drains more than one task per await.
    scheduler::reset_for_test();

    COUNTER.store(0, Ordering::SeqCst);

    let handles: Vec<*mut ZoTask> = (0..50)
      .map(|_| unsafe { _zo_task_spawn(increment_counter) })
      .collect();

    for h in handles {
      unsafe { _zo_task_await(h) };
    }

    assert_eq!(COUNTER.load(Ordering::SeqCst), 50);
  }

  #[test]
  fn tasks_yield_and_resume() {
    // Each task yields once before incrementing; the
    // scheduler must re-queue voluntarily-yielded
    // tasks so they eventually make progress.
    scheduler::reset_for_test();

    COUNTER.store(0, Ordering::SeqCst);

    let handles: Vec<*mut ZoTask> = (0..10)
      .map(|_| unsafe { _zo_task_spawn(yield_then_increment) })
      .collect();

    for h in handles {
      unsafe { _zo_task_await(h) };
    }

    assert_eq!(COUNTER.load(Ordering::SeqCst), 10);
  }

  #[test]
  #[should_panic(expected = "zo-task panicked")]
  fn panicking_task_propagates_to_awaiter() {
    scheduler::reset_for_test();

    unsafe {
      let task = _zo_task_spawn(panicking_task);

      _zo_task_await(task);
    }
  }
}
