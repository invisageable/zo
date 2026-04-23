//! Phase 3 of `PLAN_PREHISTORY.md` — green-thread
//! scheduler state + primitives.
//!
//! This module owns the scheduling policy: the run
//! queue (FIFO / round-robin per D5), the "scheduler
//! context" (the resume point every task yields back
//! to), and the pointer-to-current-task slot. Task
//! identity + lifecycle live in `task.rs`.
//!
//! One scheduler state per OS thread (v1: single OS
//! thread runs everything). Stored in `thread_local!`
//! so the multi-scheduler work-stealing upgrade (D3
//! v2) is a structural addition, not a rewrite.
//!
//! External shape: `task.rs` uses [`with`] /
//! [`yield_now`] / [`drain_until_dead`] to coordinate
//! with this module. No public ABI exports live
//! here — all C-facing symbols are in `task.rs`.
//!
//! Scope boundary: channel integration (park a task
//! on a full buffer, wake it on matching recv) is a
//! follow-up. Today, `channel.rs` still uses pthread
//! Condvars — which on a single scheduler thread
//! deadlock under contention. Tests in this module
//! only exercise spawn / yield / await.

use crate::ctxsw::{Context, ctx_switch};
use crate::task::{TaskState, ZoTask};

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;

// ===== Scheduler state (thread-local) =====

/// Per-OS-thread scheduler state. All access is from
/// the single scheduler thread, so interior mutability
/// via `RefCell` / `Cell` is race-free without a
/// `Mutex`. Multi-scheduler work-stealing (D3 v2) would
/// add cross-scheduler primitives on top without
/// changing this struct.
pub struct SchedulerState {
  /// The scheduler's "resume point" — every task yields
  /// back here. Written on each `ctx_switch(scheduler,
  /// task)`, read on each `ctx_switch(task, scheduler)`.
  ctx: RefCell<Context>,
  /// Runnable tasks in FIFO order (D5 — round-robin).
  run_queue: RefCell<VecDeque<*mut ZoTask>>,
  /// Task currently executing on this OS thread, or
  /// `None` when the scheduler loop is between tasks.
  current: Cell<Option<*mut ZoTask>>,
}

impl SchedulerState {
  const fn new() -> Self {
    Self {
      ctx: RefCell::new(Context::zeroed()),
      run_queue: RefCell::new(VecDeque::new()),
      current: Cell::new(None),
    }
  }

  /// Returns the currently running task pointer, if a
  /// task is executing.
  pub fn current(&self) -> Option<*mut ZoTask> {
    self.current.get()
  }

  /// Pushes `task` onto the back of the run queue. The
  /// scheduler will pick it up on the next `run_one`.
  pub fn enqueue(&self, task: *mut ZoTask) {
    self.run_queue.borrow_mut().push_back(task);
  }

  /// Raw pointer to the scheduler context. For the
  /// `ctx_switch(task → scheduler)` call site in
  /// `task.rs::exit_current` and elsewhere.
  pub fn scheduler_ctx_ptr(&self) -> *mut Context {
    self.ctx.as_ptr()
  }

  fn pop_ready(&self) -> Option<*mut ZoTask> {
    self.run_queue.borrow_mut().pop_front()
  }

  fn set_current(&self, task: Option<*mut ZoTask>) {
    self.current.set(task);
  }

  /// Pop a locally-queued task, if any. Used by pool
  /// workers (`pool.rs`) to drain re-queued yielded /
  /// woken tasks from the thread-local run queue.
  pub fn pop_local(&self) -> Option<*mut ZoTask> {
    self.pop_ready()
  }

  /// Whether the thread-local run queue has any ready
  /// tasks. Used by pool workers to decide whether to
  /// steal or park.
  pub fn local_is_empty(&self) -> bool {
    self.run_queue.borrow().is_empty()
  }
}

thread_local! {
  static SCHED: SchedulerState = const { SchedulerState::new() };
}

/// Run `f` with a reference to this thread's
/// scheduler state. Borrowed shape mirrors the
/// stdlib's `thread_local!::with` — callers can't
/// store the reference past the closure.
pub fn with<R>(f: impl FnOnce(&SchedulerState) -> R) -> R {
  SCHED.with(f)
}

// ===== Yield / run-one / drain =====

/// Yield control from the currently-running task back
/// to the scheduler. Saves the task's CPU state into
/// its own `ctx` and loads the scheduler's `ctx`. The
/// scheduler decides whether to re-queue or drop the
/// task based on its `state` field.
///
/// # Safety
///
/// Must be called only from a task's execution — i.e.
/// `current()` returns `Some`. Calling from the
/// scheduler thread's non-task code is a logic bug
/// and triggers a panic here instead of silently
/// switching into unrelated state.
pub unsafe fn yield_now() {
  with(|s| {
    let task = s
      .current()
      .expect("yield_now called outside a task context");

    // We're still `Running` — the scheduler's `run_one`
    // caller observes post-switch state and re-queues
    // anything still runnable (voluntary yield).

    // SAFETY: `task` pointer is live while Running;
    // exclusive access from this OS thread.
    let task_ctx = unsafe { &raw mut (*task).ctx };
    let sch_ctx = s.scheduler_ctx_ptr();

    unsafe {
      ctx_switch(task_ctx, sch_ctx);
    }
  });
}

/// Drains the run queue until `until_dead`'s state
/// transitions to `Dead`. Called from non-task code
/// (typically `main` inside a nursery) to block on
/// child completion.
///
/// # Safety
///
/// `until_dead` must be a valid `*mut ZoTask` handle
/// whose backing `Box<ZoTask>` outlives this call.
pub unsafe fn drain_until_dead(until_dead: *mut ZoTask) {
  loop {
    // SAFETY: caller contract — pointer still live.
    if unsafe { (*until_dead).state } == TaskState::Dead {
      return;
    }

    let next = with(|s| s.pop_ready());

    match next {
      Some(task) => unsafe { run_one(task) },
      None => {
        // Awaited task isn't Dead and no one else is
        // runnable — genuine deadlock. Panic loudly;
        // a silent stall is worse than a loud abort.
        panic!(
          "zo-scheduler deadlock: awaited task is Blocked \
           but run queue is empty",
        );
      }
    }
  }
}

/// Public entry point that lets external runners
/// (pool workers in `pool.rs`) drive a task to its next
/// yield/die point on the current OS thread's scheduler.
///
/// # Safety
///
/// Same contract as [`run_one`] — `task` must be a live
/// pointer transitioning out of `Ready`, and the caller
/// must NOT already be inside a task (no re-entrancy).
pub unsafe fn run_one_external(task: *mut ZoTask) {
  unsafe { run_one(task) };
}

/// Switch from scheduler context into `task`, block
/// until it yields / dies, then re-queue based on the
/// post-switch state.
///
/// # Safety
///
/// `task` must be a valid `*mut ZoTask` with `state`
/// transitioning out of `Ready`. Only called from
/// scheduler-loop context (not from inside a task).
unsafe fn run_one(task: *mut ZoTask) {
  // SAFETY: caller contract.
  unsafe {
    (*task).state = TaskState::Running;
  }

  with(|s| {
    s.set_current(Some(task));

    let sch_ctx = s.scheduler_ctx_ptr();
    // SAFETY: task pointer valid for the duration.
    let task_ctx = unsafe { &raw mut (*task).ctx };

    // Enter the task — returns when it yields / dies.
    unsafe {
      ctx_switch(sch_ctx, task_ctx);
    }

    s.set_current(None);

    // SAFETY: caller contract — pointer still live.
    let state = unsafe { (*task).state };

    match state {
      TaskState::Running => {
        // Voluntary yield — still runnable.
        // SAFETY: pointer valid, state transition is
        // exclusive to this thread.
        unsafe { (*task).state = TaskState::Ready };

        s.enqueue(task);
      }
      TaskState::Ready => {
        // Explicit self-requeue by caller; honor.
        s.enqueue(task);
      }
      TaskState::Blocked => {
        // Someone else (channel wait, waiter registry)
        // re-queues when the block resolves.
      }
      TaskState::Dead => {
        // `task::exit_current` already notified
        // waiters. Struct lives until its `Box`
        // handle is reclaimed by `await_task`.
      }
    }
  });
}

// ===== Test helpers =====

/// Clears thread-local scheduler state between tests
/// so residue from one test (aborted panic path,
/// leftover queue entries) doesn't leak into the
/// next. No-op if the queue is already empty.
///
/// Exposed unconditionally (not `#[cfg(test)]`) so
/// integration tests in `tests/` can call it — they
/// compile against the non-test crate.
pub fn reset_for_test() {
  with(|s| {
    s.run_queue.borrow_mut().clear();
    s.current.set(None);
  });
}
