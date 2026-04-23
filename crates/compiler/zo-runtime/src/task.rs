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

use crate::ctxsw::{Context, ctx_switch};
use crate::scheduler;

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex};
use std::thread;

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

/// A task — either a green task multiplexed on the
/// scheduler or a dedicated OS thread. The two-tier
/// spawn model from `PLAN_PREHISTORY.md` Phase 4
/// surfaces both through the same `*mut ZoTask`
/// handle; `threaded` is the discriminator.
///
/// Most fields are green-task-only. For a threaded
/// task, `ctx` / `state` / `_stack` / `waiters` carry
/// default / empty values — the OS thread runs the
/// user callee directly without any scheduler
/// involvement, so the scheduler fields are
/// unused. Keeping them on the struct (with a ~zero
/// cost for an empty `Box<[u8]>`) avoids branching
/// every scheduler access on the task kind.
pub struct ZoTask {
  /// Saved CPU state — restored on every resume, saved
  /// on every yield. Green-only.
  pub ctx: Context,
  /// Where the task is in its lifecycle. Green-only.
  pub state: TaskState,
  /// How the task ended, once `state == Dead`.
  /// Green-only; threaded tasks carry their outcome
  /// inside `threaded.outcome` because it transitions
  /// from a different OS thread.
  pub outcome: TaskOutcome,
  /// Task-owned stack. Green-only; threaded tasks use
  /// their pthread's stack and leave this field as an
  /// empty `Box<[u8]>` (zero heap bytes).
  _stack: Box<[u8]>,
  /// User callee address, passed via `x20` to the task
  /// shim on first enter. Stored to survive the Box's
  /// move during construction. Green-only.
  user_entry_addr: u64,
  /// Tasks that have parked on `await`-ing this one.
  /// Green-only.
  pub waiters: Vec<*mut ZoTask>,
  /// Threaded-kind extension — `Some` when this task
  /// owns a pthread running the user callee; `None`
  /// for the common green-task case.
  threaded: Option<ThreadedData>,
}

/// Extra state for a threaded task. The `join` handle
/// lives until `_zo_task_await` consumes it; `outcome`
/// is written by the spawned pthread before it exits
/// and read back on await.
struct ThreadedData {
  /// pthread handle — `Some` from spawn to await;
  /// `take()`n during await.
  join: Mutex<Option<thread::JoinHandle<()>>>,
  /// Terminal outcome, set by the pthread's own
  /// `catch_unwind` frame before the thread exits.
  outcome: Arc<Mutex<TaskOutcome>>,
}

impl ZoTask {
  /// Allocate a new green task. The task is `Ready`
  /// and its Context bootstraps into [`task_shim`] on
  /// the first `ctx_switch` into it.
  fn new_green(user_entry: extern "C-unwind" fn()) -> Box<Self> {
    let mut stack = vec![0u8; DEFAULT_STACK_SIZE].into_boxed_slice();
    let stack_top = unsafe { stack.as_mut_ptr().add(stack.len()) };

    let mut task = Box::new(Self {
      ctx: Context::zeroed(),
      state: TaskState::Ready,
      outcome: TaskOutcome::Running,
      _stack: stack,
      user_entry_addr: user_entry as *const () as u64,
      waiters: Vec::new(),
      threaded: None,
    });

    // Carry the task's own address through to the
    // shim, so the shim can read `user_entry_addr` and
    // flip `state` / `outcome` at the end.
    let task_addr = &mut *task as *mut ZoTask as u64;

    task.ctx.bootstrap(stack_top, task_shim, task_addr);

    task
  }

  /// Allocate a new threaded task. The caller is
  /// expected to follow up with [`spawn_thread`], which
  /// creates the pthread and records its `JoinHandle`.
  fn new_threaded_shell() -> Box<Self> {
    Box::new(Self {
      // Scheduler fields unused for threaded tasks;
      // defaults keep them cheap.
      ctx: Context::zeroed(),
      state: TaskState::Ready,
      outcome: TaskOutcome::Running,
      _stack: Box::<[u8]>::default(),
      user_entry_addr: 0,
      waiters: Vec::new(),
      threaded: Some(ThreadedData {
        join: Mutex::new(None),
        outcome: Arc::new(Mutex::new(TaskOutcome::Running)),
      }),
    })
  }

  /// True when this task is backed by an OS thread
  /// (`spawn thread fn()`); false for the common
  /// green-task case.
  pub fn is_threaded(&self) -> bool {
    self.threaded.is_some()
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
  let task = Box::into_raw(ZoTask::new_green(user_entry));

  scheduler::with(|s| s.enqueue(task));

  task
}

/// Spawn a new threaded task. Creates a dedicated OS
/// thread (via `std::thread::spawn`, which uses
/// `pthread_create` on POSIX) that runs `user_entry`
/// directly — no scheduler involvement. Panics in the
/// callee are caught and stored in the task's
/// `outcome`, re-raised on the awaiter.
///
/// # Safety
///
/// `user_entry` must be a live function pointer; the
/// returned handle is consumed by [`await_task`] or
/// equivalent.
pub unsafe fn spawn_thread(user_entry: extern "C-unwind" fn()) -> *mut ZoTask {
  let task = Box::into_raw(ZoTask::new_threaded_shell());

  // SAFETY: `task` was just Box::into_raw'd, pointer
  // is valid until the await consumes it; `threaded`
  // is Some by construction of `new_threaded_shell`.
  let threaded = unsafe { (*task).threaded.as_ref().unwrap() };
  let outcome = Arc::clone(&threaded.outcome);

  let join = thread::spawn(move || {
    // `catch_unwind` needs `FnOnce()`; wrap the extern
    // fn in a closure. Zero captures.
    let result = catch_unwind(AssertUnwindSafe(|| user_entry()));

    *outcome.lock().expect("zo-task threaded outcome poisoned") = match result {
      Ok(()) => TaskOutcome::Completed,
      Err(_) => TaskOutcome::Panicked,
    };
  });

  *threaded
    .join
    .lock()
    .expect("zo-task threaded join poisoned") = Some(join);

  task
}

/// Await `target`. Handles both kinds:
///
/// - **Threaded**: pthread-join the OS thread. Read
///   the stored outcome; re-raise if `Panicked`.
/// - **Green**: if called from another task, park the
///   caller until the target dies; if called from the
///   non-task scheduler thread (main inside a nursery),
///   drain the run queue.
///
/// # Safety
///
/// `target` must be a `*mut ZoTask` produced by
/// [`spawn`] or [`spawn_thread`] (or their C ABI
/// equivalents) and not yet freed.
pub unsafe fn await_task(target: *mut ZoTask) {
  // Threaded path — pthread_join; no scheduler.
  // SAFETY: caller contract.
  if unsafe { (*target).is_threaded() } {
    // SAFETY: same.
    let threaded = unsafe { (*target).threaded.as_ref().unwrap() };

    let join = threaded
      .join
      .lock()
      .expect("zo-task threaded join poisoned")
      .take();

    if let Some(handle) = join {
      // The `catch_unwind` inside the thread already
      // stored the outcome; the Err here only fires
      // if the thread itself (not the user callee)
      // somehow panicked — propagate that directly.
      if let Err(payload) = handle.join() {
        std::panic::resume_unwind(payload);
      }
    }

    let outcome_snapshot = *threaded
      .outcome
      .lock()
      .expect("zo-task threaded outcome poisoned");

    // SAFETY: exclusive ownership transfers now.
    drop(unsafe { Box::from_raw(target) });

    if matches!(outcome_snapshot, TaskOutcome::Panicked) {
      panic!("zo-task panicked — propagating to awaiter");
    }

    return;
  }

  // Green path (existing).
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

/// Spawn a new OS-thread-backed task (`spawn thread
/// fn()` in source). Returns an opaque handle
/// consumed by `_zo_task_await`.
///
/// # Safety
///
/// `callee` must be a live `extern "C-unwind"`
/// function pointer. The returned handle must be
/// consumed by `_zo_task_await`.
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn _zo_task_spawn_thread(
  callee: extern "C-unwind" fn(),
) -> *mut ZoTask {
  unsafe { spawn_thread(callee) }
}

/// Await a task. Blocks the caller until the task
/// completes. Re-raises a task panic on the caller.
/// Handles both green and threaded tasks.
///
/// # Safety
///
/// `task` must have come from `_zo_task_spawn` or
/// `_zo_task_spawn_thread` and not yet been awaited.
/// After this call returns, `task` is freed and must
/// not be referenced again.
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

  // ===== PLAN_PREHISTORY Phase 4 — threaded spawn =====

  #[test]
  fn threaded_spawn_runs_on_dedicated_os_thread() {
    // A threaded task runs on its own OS thread — no
    // scheduler involvement. This proves the `spawn
    // thread fn()` surface path works end-to-end:
    // pthread_create → callee → pthread_join.
    COUNTER.store(0, Ordering::SeqCst);

    unsafe {
      let task = _zo_task_spawn_thread(increment_counter);

      assert!((*task).is_threaded());

      _zo_task_await(task);
    }

    assert_eq!(COUNTER.load(Ordering::SeqCst), 1);
  }

  #[test]
  #[should_panic(expected = "zo-task panicked")]
  fn panicking_threaded_task_propagates_to_awaiter() {
    unsafe {
      let task = _zo_task_spawn_thread(panicking_task);

      _zo_task_await(task);
    }
  }

  #[test]
  fn green_and_threaded_tasks_coexist() {
    // Spawn both kinds; each awaits independently. No
    // shared state between them here — just that the
    // two paths don't interfere.
    scheduler::reset_for_test();

    COUNTER.store(0, Ordering::SeqCst);

    unsafe {
      let green = _zo_task_spawn(increment_counter);
      let threaded = _zo_task_spawn_thread(increment_counter);

      _zo_task_await(green);
      _zo_task_await(threaded);
    }

    assert_eq!(COUNTER.load(Ordering::SeqCst), 2);
  }
}
