//! Green-task lifecycle.
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
//! `_zo_task_spawn` / `_zo_task_await` are the stable
//! ABI symbols the ARM codegen's BL placeholders
//! resolve against — compiled programs don't know
//! whether they run on green tasks or OS threads.

use crate::ctxsw::{Context, ctx_switch};
use crate::scheduler;
use crate::stack::TaskStack;

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

/// Lifecycle states a task moves through. The state
/// machine is driven entirely by the scheduler thread
/// (single OS thread per scheduler).
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
/// We store the outcome rather than unwinding across
/// the FFI boundary — the caller of `_zo_task_await`
/// re-raises if `Panicked`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskOutcome {
  /// Body hasn't returned yet.
  Running,
  /// Normal return. The current shape doesn't thread
  /// the return value back; a richer
  /// `Completed(Vec<u8>)` would wire the type-aware
  /// return path.
  Completed,
  /// Body panicked. Payload is currently discarded;
  /// carrying it would require a Box<dyn Any> in the
  /// task struct.
  Panicked,
}

/// A task — either a green task multiplexed on the
/// scheduler or a dedicated OS thread. Two-tier spawn
/// surfaces both kinds through the same `*mut ZoTask`
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
  /// Task-owned stack. Virtual reservation with a
  /// growable committed prefix. Lives inline inside
  /// the `Box<ZoTask>` heap allocation — its address
  /// is stable for the task's lifetime, which is what
  /// the fault-handler registry requires. `None` for
  /// threaded tasks, which run on the pthread's own
  /// kernel-managed stack.
  _stack: Option<TaskStack>,
  /// User callee address, passed via `x20` to the task
  /// shim on first enter. Stored to survive the Box's
  /// move during construction. Green-only.
  user_entry_addr: u64,
  /// Tasks that have parked on `await`-ing this one.
  /// Green-only.
  pub waiters: Vec<*mut ZoTask>,
  /// Cancellation flag. Set by `_zo_task_cancel`;
  /// queried by `_zo_task_is_cancelled` and (future)
  /// yield-site cancellation polls. Inline — the
  /// supervisor already holds a `*mut ZoTask` handle
  /// and writes through it directly; no Arc overhead.
  pub cancelled: AtomicBool,
  /// Arguments stashed by `_zo_task_spawn_N` and
  /// replayed into `X0..X(N-1)` by the matching
  /// `task_shim_N` before jumping to the user
  /// callee. `0` for the zero-arg spawn. Green-only.
  user_arg0: u64,
  user_arg1: u64,
  user_arg2: u64,
  /// Return value captured by the task shim when the
  /// user callee completes normally. Read back by
  /// `_zo_task_await` and returned through X0. For
  /// void-returning user fns, the shim transmutes to
  /// `fn(..) -> u64` anyway — the extra u64 in X0 at
  /// return is simply ignored by the caller. Green-
  /// only (threaded tasks don't surface a value yet).
  ret_value: u64,
  /// Threaded-kind extension — `Some` when this task
  /// owns a pthread running the user callee; `None`
  /// for the common green-task case.
  threaded: Option<Box<ThreadedData>>,
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
  /// Allocate a new green task without enqueuing it on
  /// any scheduler. Public so `pool.rs` and
  /// cost-decomposition microbenches in `tests/` can
  /// build tasks without touching the thread-local
  /// run queue.
  pub fn new_green_standalone(user_entry: extern "C-unwind" fn()) -> Box<Self> {
    Self::new_green(user_entry)
  }

  /// Allocate a new green task. The task is `Ready`
  /// and its Context bootstraps into [`task_shim`] on
  /// the first `ctx_switch` into it.
  fn new_green(user_entry: extern "C-unwind" fn()) -> Box<Self> {
    let stack = TaskStack::reserve();
    let stack_top = stack.top();

    let mut task = Box::new(Self {
      ctx: Context::zeroed(),
      state: TaskState::Ready,
      outcome: TaskOutcome::Running,
      _stack: Some(stack),
      user_entry_addr: user_entry as *const () as u64,
      waiters: Vec::new(),
      cancelled: AtomicBool::new(false),
      user_arg0: 0,
      user_arg1: 0,
      user_arg2: 0,
      ret_value: 0,
      threaded: None,
    });

    register_task_stack(&task);

    // Carry the task's own address through to the
    // shim, so the shim can read `user_entry_addr` and
    // flip `state` / `outcome` at the end.
    let task_addr = &mut *task as *mut ZoTask as u64;

    task.ctx.bootstrap(stack_top, task_shim, task_addr);

    task
  }

  /// Variant for spawning a 1-arg callee — the
  /// bootstrapped context enters `task_shim_1`, which
  /// pulls `user_arg0` from the task before jumping to
  /// `user_entry(arg0)`.
  fn new_green_1(
    user_entry: extern "C-unwind" fn(u64),
    arg0: u64,
  ) -> Box<Self> {
    let stack = TaskStack::reserve();
    let stack_top = stack.top();

    let mut task = Box::new(Self {
      ctx: Context::zeroed(),
      state: TaskState::Ready,
      outcome: TaskOutcome::Running,
      _stack: Some(stack),
      user_entry_addr: user_entry as *const () as u64,
      waiters: Vec::new(),
      cancelled: AtomicBool::new(false),
      user_arg0: arg0,
      user_arg1: 0,
      user_arg2: 0,
      ret_value: 0,
      threaded: None,
    });

    register_task_stack(&task);

    let task_addr = &mut *task as *mut ZoTask as u64;

    task.ctx.bootstrap(stack_top, task_shim_1, task_addr);

    task
  }

  /// Variant for spawning a 2-arg callee.
  fn new_green_2(
    user_entry: extern "C-unwind" fn(u64, u64),
    arg0: u64,
    arg1: u64,
  ) -> Box<Self> {
    let stack = TaskStack::reserve();
    let stack_top = stack.top();

    let mut task = Box::new(Self {
      ctx: Context::zeroed(),
      state: TaskState::Ready,
      outcome: TaskOutcome::Running,
      _stack: Some(stack),
      user_entry_addr: user_entry as *const () as u64,
      waiters: Vec::new(),
      cancelled: AtomicBool::new(false),
      user_arg0: arg0,
      user_arg1: arg1,
      user_arg2: 0,
      ret_value: 0,
      threaded: None,
    });

    register_task_stack(&task);

    let task_addr = &mut *task as *mut ZoTask as u64;

    task.ctx.bootstrap(stack_top, task_shim_2, task_addr);

    task
  }

  /// Variant for spawning a 3-arg callee.
  fn new_green_3(
    user_entry: extern "C-unwind" fn(u64, u64, u64),
    arg0: u64,
    arg1: u64,
    arg2: u64,
  ) -> Box<Self> {
    let stack = TaskStack::reserve();
    let stack_top = stack.top();

    let mut task = Box::new(Self {
      ctx: Context::zeroed(),
      state: TaskState::Ready,
      outcome: TaskOutcome::Running,
      _stack: Some(stack),
      user_entry_addr: user_entry as *const () as u64,
      waiters: Vec::new(),
      cancelled: AtomicBool::new(false),
      user_arg0: arg0,
      user_arg1: arg1,
      user_arg2: arg2,
      ret_value: 0,
      threaded: None,
    });

    register_task_stack(&task);

    let task_addr = &mut *task as *mut ZoTask as u64;

    task.ctx.bootstrap(stack_top, task_shim_3, task_addr);

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
      _stack: None,
      user_entry_addr: 0,
      waiters: Vec::new(),
      cancelled: AtomicBool::new(false),
      user_arg0: 0,
      user_arg1: 0,
      user_arg2: 0,
      ret_value: 0,
      threaded: Some(Box::new(ThreadedData {
        join: Mutex::new(None),
        outcome: Arc::new(Mutex::new(TaskOutcome::Running)),
      })),
    })
  }

  /// True when this task is backed by an OS thread
  /// (`spawn thread fn()`); false for the common
  /// green-task case.
  pub fn is_threaded(&self) -> bool {
    self.threaded.is_some()
  }
}

impl Drop for ZoTask {
  fn drop(&mut self) {
    // Pull the registration before the stack's address
    // can change, then move the stack into the pool.
    // Threaded tasks never register (no `_stack`), so
    // this is a no-op for them.
    if let Some(stack) = self._stack.as_ref() {
      stack.unregister();
    }

    if let Some(stack) = self._stack.take() {
      stack.recycle();
    }
  }
}

/// Publish a freshly-spawned green task's stack to the
/// fault-handler registry. Called once after the
/// `Box<ZoTask>` is built, so the registered pointer
/// points at a heap slot that won't move for the task's
/// lifetime. No-op on a task whose `_stack` is `None`
/// (i.e. the threaded shell, which never sets it).
fn register_task_stack(task: &ZoTask) {
  if let Some(stack) = task._stack.as_ref() {
    stack.register();
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

  // Transmute the user callee as `fn() -> u64`. Zo
  // callees compile with the arm64 C ABI where a
  // return value is left in X0 regardless of the
  // static `unit` / `int` / pointer distinction —
  // reading X0 as `u64` is safe for every return
  // width the ABI emits.
  //
  // SAFETY: `user_entry_addr` was constructed from a
  // valid `extern "C-unwind" fn()` pointer; the arm64
  // ABI allows narrowing the return type between the
  // declared and the transmuted signatures provided
  // the caller only reads the returned X0 bits.
  let user_entry: extern "C-unwind" fn() -> u64 = unsafe {
    std::mem::transmute::<*const (), extern "C-unwind" fn() -> u64>(
      user_entry_addr as *const (),
    )
  };

  // `catch_unwind` needs a `FnOnce()` — wrap so the
  // `u64` return threads through the `Ok` arm.
  let result = catch_unwind(AssertUnwindSafe(|| user_entry()));

  // SAFETY: same task pointer as above, still live.
  unsafe {
    match result {
      Ok(v) => {
        (*task).ret_value = v;
        (*task).outcome = TaskOutcome::Completed;
      }
      Err(_) => {
        (*task).outcome = TaskOutcome::Panicked;
      }
    }
  }

  exit_current();
}

/// 1-arg shim — reads `user_entry_addr` + `user_arg0`
/// from the task, transmutes the address to a 1-arg
/// function pointer, and calls it under `catch_unwind`.
extern "C-unwind" fn task_shim_1(task_addr: u64) {
  let task = task_addr as *mut ZoTask;
  // SAFETY: `task_addr` carries a live `*mut ZoTask` —
  // the box outlives this shim's call frame.
  let (user_entry_addr, arg0) =
    unsafe { ((*task).user_entry_addr, (*task).user_arg0) };

  // SAFETY: `user_entry_addr` was built from a valid
  // `extern "C-unwind" fn(u64)` pointer; the arm64 ABI
  // makes reading X0 as `u64` safe for any return
  // width the callee emits (see `task_shim`).
  let user_entry: extern "C-unwind" fn(u64) -> u64 = unsafe {
    std::mem::transmute::<*const (), extern "C-unwind" fn(u64) -> u64>(
      user_entry_addr as *const (),
    )
  };

  let result = catch_unwind(AssertUnwindSafe(|| user_entry(arg0)));

  unsafe {
    match result {
      Ok(v) => {
        (*task).ret_value = v;
        (*task).outcome = TaskOutcome::Completed;
      }
      Err(_) => {
        (*task).outcome = TaskOutcome::Panicked;
      }
    }
  }

  exit_current();
}

/// 2-arg shim — same pattern as `task_shim_1`.
extern "C-unwind" fn task_shim_2(task_addr: u64) {
  let task = task_addr as *mut ZoTask;
  let (user_entry_addr, arg0, arg1) = unsafe {
    (
      (*task).user_entry_addr,
      (*task).user_arg0,
      (*task).user_arg1,
    )
  };

  let user_entry: extern "C-unwind" fn(u64, u64) -> u64 = unsafe {
    std::mem::transmute::<*const (), extern "C-unwind" fn(u64, u64) -> u64>(
      user_entry_addr as *const (),
    )
  };

  let result = catch_unwind(AssertUnwindSafe(|| user_entry(arg0, arg1)));

  unsafe {
    match result {
      Ok(v) => {
        (*task).ret_value = v;
        (*task).outcome = TaskOutcome::Completed;
      }
      Err(_) => {
        (*task).outcome = TaskOutcome::Panicked;
      }
    }
  }

  exit_current();
}

/// 3-arg shim — same pattern as `task_shim_1`.
extern "C-unwind" fn task_shim_3(task_addr: u64) {
  let task = task_addr as *mut ZoTask;
  let (user_entry_addr, arg0, arg1, arg2) = unsafe {
    (
      (*task).user_entry_addr,
      (*task).user_arg0,
      (*task).user_arg1,
      (*task).user_arg2,
    )
  };

  let user_entry: extern "C-unwind" fn(u64, u64, u64) -> u64 = unsafe {
    std::mem::transmute::<*const (), extern "C-unwind" fn(u64, u64, u64) -> u64>(
      user_entry_addr as *const (),
    )
  };

  let result = catch_unwind(AssertUnwindSafe(|| user_entry(arg0, arg1, arg2)));

  unsafe {
    match result {
      Ok(v) => {
        (*task).ret_value = v;
        (*task).outcome = TaskOutcome::Completed;
      }
      Err(_) => {
        (*task).outcome = TaskOutcome::Panicked;
      }
    }
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

    // Release any TLS entries this task stored before
    // the `Box<ZoTask>` gets reclaimed — otherwise the
    // process-wide `TASK_TLS` side table leaks every
    // dead task's slots forever.
    crate::tls::clear_for_task(task);

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

/// Spawn a 1-arg green task. `arg0` is the single
/// callee argument, passed through to `user_entry` by
/// `task_shim_1` on first scheduler dispatch.
///
/// # Safety
///
/// Same contract as [`spawn`] plus: `user_entry` must
/// be a live `extern "C-unwind" fn(u64)`.
pub unsafe fn spawn_1(
  user_entry: extern "C-unwind" fn(u64),
  arg0: u64,
) -> *mut ZoTask {
  let task = Box::into_raw(ZoTask::new_green_1(user_entry, arg0));

  scheduler::with(|s| s.enqueue(task));

  task
}

/// Spawn a 2-arg green task. See [`spawn_1`].
///
/// # Safety
///
/// `user_entry` must be a live
/// `extern "C-unwind" fn(u64, u64)`.
pub unsafe fn spawn_2(
  user_entry: extern "C-unwind" fn(u64, u64),
  arg0: u64,
  arg1: u64,
) -> *mut ZoTask {
  let task = Box::into_raw(ZoTask::new_green_2(user_entry, arg0, arg1));

  scheduler::with(|s| s.enqueue(task));

  task
}

/// Spawn a 3-arg green task. See [`spawn_1`].
///
/// # Safety
///
/// `user_entry` must be a live
/// `extern "C-unwind" fn(u64, u64, u64)`.
pub unsafe fn spawn_3(
  user_entry: extern "C-unwind" fn(u64, u64, u64),
  arg0: u64,
  arg1: u64,
  arg2: u64,
) -> *mut ZoTask {
  let task = Box::into_raw(ZoTask::new_green_3(user_entry, arg0, arg1, arg2));

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
pub unsafe fn await_task(target: *mut ZoTask) -> u64 {
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

    // Threaded tasks don't capture a return value
    // yet — the OS thread doesn't go through the
    // shim's `catch_unwind(|| user_entry())` path.
    return 0;
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

  task.ret_value
}

// ===== C ABI exports =====
//
// ARM codegen emits `BL _zo_task_*` placeholders that
// resolve against these symbols at link time.

/// Spawn a new green task. Returns an opaque handle
/// consumed by `_zo_task_await`.
///
/// # Safety
///
/// `callee` must be a live `extern "C-unwind"` function
/// pointer. The returned handle remains valid until
/// consumed by `_zo_task_await` — dropping it without
/// awaiting leaks the task's stack and ZoTask struct.
/// Drain every ready task in the thread-local
/// scheduler to completion. Called at the close of a
/// `nursery { }` (and `supervise { }`) scope so sibling
/// tasks finish before control exits the scope.
///
/// Re-entrant: safe to call from inside a green task
/// (nested-nursery case). `run_one` snapshots
/// `current` + the global scheduler-ctx slot on the
/// CPU stack and restores them when the inner task
/// yields, so the outer task's later `exit_current`
/// still sees itself as current and the global ctx
/// still points at the outer resume site.
///
/// Idempotent on an empty queue.
#[unsafe(export_name = "zo_nursery_drain")]
pub unsafe extern "C-unwind" fn _zo_nursery_drain() {
  scheduler::drain_all();
}

/// Spawn a green task.
///
/// # Safety
///
/// `callee` must be a live `extern "C-unwind"`
/// function pointer. The returned task handle is
/// consumed by a later [`_zo_task_await`] or
/// implicitly by the nursery drain at scope exit —
/// dropping it without either would leak the task's
/// stack and `ZoTask` struct.
#[unsafe(export_name = "zo_task_spawn")]
pub unsafe extern "C-unwind" fn _zo_task_spawn(
  callee: extern "C-unwind" fn(),
) -> *mut ZoTask {
  unsafe { spawn(callee) }
}

/// Spawn a 1-arg green task.
///
/// # Safety
///
/// `callee` must be a live
/// `extern "C-unwind" fn(u64)`. Same handle-lifetime
/// contract as [`_zo_task_spawn`].
#[unsafe(export_name = "zo_task_spawn_1")]
pub unsafe extern "C-unwind" fn _zo_task_spawn_1(
  callee: extern "C-unwind" fn(u64),
  arg0: u64,
) -> *mut ZoTask {
  unsafe { spawn_1(callee, arg0) }
}

/// Spawn a 2-arg green task.
///
/// # Safety
///
/// `callee` must be a live
/// `extern "C-unwind" fn(u64, u64)`.
#[unsafe(export_name = "zo_task_spawn_2")]
pub unsafe extern "C-unwind" fn _zo_task_spawn_2(
  callee: extern "C-unwind" fn(u64, u64),
  arg0: u64,
  arg1: u64,
) -> *mut ZoTask {
  unsafe { spawn_2(callee, arg0, arg1) }
}

/// Spawn a 3-arg green task.
///
/// # Safety
///
/// `callee` must be a live
/// `extern "C-unwind" fn(u64, u64, u64)`.
#[unsafe(export_name = "zo_task_spawn_3")]
pub unsafe extern "C-unwind" fn _zo_task_spawn_3(
  callee: extern "C-unwind" fn(u64, u64, u64),
  arg0: u64,
  arg1: u64,
  arg2: u64,
) -> *mut ZoTask {
  unsafe { spawn_3(callee, arg0, arg1, arg2) }
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
#[unsafe(export_name = "zo_task_spawn_thread")]
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
#[unsafe(export_name = "zo_task_await")]
pub unsafe extern "C-unwind" fn _zo_task_await(task: *mut ZoTask) -> u64 {
  if task.is_null() {
    return 0;
  }

  // SAFETY: `await_task` frees the `Box<ZoTask>` after
  // it's done; we read `ret_value` from the boxed task
  // before the drop, and return it through X0.
  unsafe { await_task(task) }
}

/// Mark `task` as cancelled. Sets an atomic flag that
/// the task itself (or a supervisor) can query via
/// [`_zo_task_is_cancelled`] and unwind cooperatively.
/// Idempotent — repeated cancels are no-ops.
///
/// # Safety
///
/// `task` must be a live handle from `_zo_task_spawn` /
/// `_zo_task_spawn_thread` that hasn't yet been awaited.
#[unsafe(export_name = "zo_task_cancel")]
pub unsafe extern "C-unwind" fn _zo_task_cancel(task: *mut ZoTask) {
  if task.is_null() {
    return;
  }

  // SAFETY: caller contract — pointer still live until
  // the matching await consumes it.
  unsafe { (*task).cancelled.store(true, Ordering::SeqCst) };
}

/// Query the cancellation flag for `task`. Returns
/// `true` if a prior [`_zo_task_cancel`] has latched
/// the flag.
///
/// # Safety
///
/// Same contract as [`_zo_task_cancel`].
#[unsafe(export_name = "zo_task_is_cancelled")]
pub unsafe extern "C-unwind" fn _zo_task_is_cancelled(
  task: *mut ZoTask,
) -> bool {
  if task.is_null() {
    return false;
  }

  // SAFETY: caller contract.
  unsafe { (*task).cancelled.load(Ordering::SeqCst) }
}

// ===== Tests =====

#[cfg(test)]
mod tests {
  use super::*;

  use std::sync::atomic::{AtomicU32, Ordering};

  /// Increments the `AtomicU32` whose address is
  /// passed in as `counter_addr`. Using a pointer-as-
  /// argument pattern (via `_zo_task_spawn_1`) lets
  /// each test own its own counter on the stack —
  /// no shared globals, so tests stay independent
  /// under parallel execution.
  extern "C-unwind" fn increment_at(counter_addr: u64) {
    let counter = counter_addr as *const AtomicU32;

    unsafe { (*counter).fetch_add(1, Ordering::SeqCst) };
  }

  extern "C-unwind" fn yield_then_increment_at(counter_addr: u64) {
    unsafe { scheduler::yield_now() };

    let counter = counter_addr as *const AtomicU32;

    unsafe { (*counter).fetch_add(1, Ordering::SeqCst) };
  }

  extern "C-unwind" fn panicking_task() {
    panic!("intentional green-task panic");
  }

  fn counter_addr(c: &AtomicU32) -> u64 {
    c as *const AtomicU32 as u64
  }

  #[test]
  fn spawn_await_runs_entry_once() {
    scheduler::reset_for_test();

    let counter = AtomicU32::new(0);

    unsafe {
      let task = _zo_task_spawn_1(increment_at, counter_addr(&counter));

      _zo_task_await(task);
    }

    assert_eq!(counter.load(Ordering::SeqCst), 1);
  }

  #[test]
  fn spawn_many_tasks_all_run() {
    // Fan-out a small batch — proves the run queue
    // drains more than one task per await.
    scheduler::reset_for_test();

    let counter = AtomicU32::new(0);
    let addr = counter_addr(&counter);

    let handles: Vec<*mut ZoTask> = (0..50)
      .map(|_| unsafe { _zo_task_spawn_1(increment_at, addr) })
      .collect();

    for h in handles {
      unsafe { _zo_task_await(h) };
    }

    assert_eq!(counter.load(Ordering::SeqCst), 50);
  }

  #[test]
  fn tasks_yield_and_resume() {
    // Each task yields once before incrementing; the
    // scheduler must re-queue voluntarily-yielded
    // tasks so they eventually make progress.
    scheduler::reset_for_test();

    let counter = AtomicU32::new(0);
    let addr = counter_addr(&counter);

    let handles: Vec<*mut ZoTask> = (0..10)
      .map(|_| unsafe { _zo_task_spawn_1(yield_then_increment_at, addr) })
      .collect();

    for h in handles {
      unsafe { _zo_task_await(h) };
    }

    assert_eq!(counter.load(Ordering::SeqCst), 10);
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

  // ===== stack-growth tests =====
  //
  // Force a green task's stack past the initial
  // committed page so the guard-page handler must
  // fire. Success: the task returns its result and
  // the runtime reports the expected extension count.

  static DEPTH_REACHED: AtomicU32 = AtomicU32::new(0);

  extern "C-unwind" fn recurse_deep(depth_as_u64: u64) {
    fn inner(n: u32) -> u32 {
      // Each frame reserves a fixed local so the
      // compiler cannot tail-call-optimize this into a
      // loop — we need real stack growth, not a single
      // frame that mutates a counter.
      let scratch: [u8; 256] = [0; 256];

      if n == 0 {
        DEPTH_REACHED.store(
          std::hint::black_box(&scratch).len() as u32,
          Ordering::SeqCst,
        );

        return 0;
      }

      std::hint::black_box(&scratch);

      inner(n - 1) + 1
    }

    let d = inner(depth_as_u64 as u32);

    DEPTH_REACHED.store(d, Ordering::SeqCst);
  }

  #[test]
  fn green_task_stack_grows_on_deep_recursion() {
    scheduler::reset_for_test();

    DEPTH_REACHED.store(0, Ordering::SeqCst);

    // 1000 frames × 256 bytes each ≈ 256 KB of stack —
    // well past the initial one-page commit but well
    // inside the 8 MB reservation cap. Reaching depth
    // 1000 is only possible if the fault handler
    // successfully grew the committed prefix multiple
    // times; a broken handler would either crash on
    // the first guard-page write or loop forever.
    unsafe {
      let task = _zo_task_spawn_1(recurse_deep, 1000);

      _zo_task_await(task);
    }

    assert_eq!(DEPTH_REACHED.load(Ordering::SeqCst), 1000);
  }

  // ===== threaded-spawn tests =====

  extern "C-unwind" fn noop_thread_entry() {}

  #[test]
  fn threaded_spawn_runs_on_dedicated_os_thread() {
    // A threaded task runs on its own OS thread — no
    // scheduler involvement. This proves the `spawn
    // thread fn()` surface path works end-to-end:
    // pthread_create → callee → pthread_join. Success
    // is `await` returning after the pthread exits;
    // the join acts as the synchronization proof.
    unsafe {
      let task = _zo_task_spawn_thread(noop_thread_entry);

      assert!((*task).is_threaded());

      _zo_task_await(task);
    }
  }

  #[test]
  #[should_panic(expected = "zo-task panicked")]
  fn panicking_threaded_task_propagates_to_awaiter() {
    unsafe {
      let task = _zo_task_spawn_thread(panicking_task);

      _zo_task_await(task);
    }
  }

  // ===== cancellation tests =====

  #[test]
  fn cancel_sets_latched_flag() {
    scheduler::reset_for_test();

    let counter = AtomicU32::new(0);

    unsafe {
      let task = _zo_task_spawn_1(increment_at, counter_addr(&counter));

      assert!(!_zo_task_is_cancelled(task));

      _zo_task_cancel(task);

      assert!(_zo_task_is_cancelled(task));

      // Idempotent — re-cancel stays true.
      _zo_task_cancel(task);
      assert!(_zo_task_is_cancelled(task));

      _zo_task_await(task);
    }
  }

  #[test]
  fn cancel_null_is_safe_noop() {
    unsafe {
      _zo_task_cancel(std::ptr::null_mut());
      assert!(!_zo_task_is_cancelled(std::ptr::null_mut()));
    }
  }

  #[test]
  fn green_and_threaded_tasks_coexist() {
    // Spawn both kinds; each awaits independently.
    // The green side runs through the scheduler; the
    // threaded side takes the pthread path. Success
    // is: both `await` calls return, and the green
    // task observably incremented its counter.
    scheduler::reset_for_test();

    let counter = AtomicU32::new(0);

    unsafe {
      let green = _zo_task_spawn_1(increment_at, counter_addr(&counter));
      let threaded = _zo_task_spawn_thread(noop_thread_entry);

      _zo_task_await(green);
      _zo_task_await(threaded);
    }

    assert_eq!(counter.load(Ordering::SeqCst), 1);
  }
}
