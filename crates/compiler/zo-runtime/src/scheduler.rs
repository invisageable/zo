//! Green-thread scheduler state + primitives.
//!
//! Owns scheduling policy: the FIFO run queue, the
//! "scheduler context" (the resume point every task
//! yields back to), and the pointer-to-current-task
//! slot. Task identity + lifecycle live in `task.rs`.
//!
//! One scheduler state per OS thread, stored in
//! `thread_local!` so a multi-scheduler work-stealing
//! layer (see `pool.rs`) can slot on top without
//! changing this module.
//!
//! External shape: `task.rs` uses [`with`] /
//! [`yield_now`] / [`drain_until_dead`] to coordinate
//! with this module. No public ABI exports live
//! here — all C-facing symbols are in `task.rs`.

use crate::channel::PthreadPark;
use crate::ctxsw::{Context, ctx_switch};
use crate::net::Selector;
use crate::task::{TaskState, ZoTask};

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::os::raw::c_int;
use std::time::Duration;

/// Opportunistic Selector poll fires once every N
/// scheduler ticks instead of every dispatch. Each
/// `kevent` / `epoll_wait` syscall costs 200-500 ns of
/// kernel transition overhead; polling every tick
/// doubled context-switch latency for pure-channel
/// workloads that never touch I/O. 64 ticks gives
/// I/O-parked tasks a ~3 µs worst-case wake latency
/// (64 × ~50 ns per ctx_switch) while eliminating 98%
/// of the syscall traffic.
const SELECTOR_POLL_INTERVAL: u32 = 64;

// ===== Scheduler state (thread-local) =====

/// Per-OS-thread scheduler state. All access is from
/// the single scheduler thread, so interior mutability
/// via `RefCell` / `Cell` is race-free without a
/// `Mutex`. A multi-scheduler work-stealing layer
/// adds cross-scheduler primitives on top without
/// changing this struct (see `pool.rs`).
pub struct SchedulerState {
  /// The scheduler's "resume point" — every task yields
  /// back here. Written on each `ctx_switch(scheduler,
  /// task)`, read on each `ctx_switch(task, scheduler)`.
  ctx: RefCell<Context>,
  /// Runnable tasks in FIFO round-robin order.
  run_queue: RefCell<VecDeque<*mut ZoTask>>,
  /// Task currently executing on this OS thread, or
  /// `None` when the scheduler loop is between tasks.
  current: Cell<Option<*mut ZoTask>>,
  /// OS-multiplexer-backed readiness queue for tasks
  /// parked on I/O. Lazily initialized on first access
  /// so the thread-local can remain const-constructed
  /// — `Selector::new` makes a syscall.
  selector: RefCell<Option<Selector>>,
  /// Reusable batch buffer for `Selector::poll`. The
  /// run-loop's `drain_ready` fires on every quantum;
  /// recycling one `Vec` here keeps the allocator out
  /// of the hot path.
  poll_batch: RefCell<Vec<*mut ZoTask>>,
  /// Scheduler tick counter. Incremented once per
  /// `run_one` dispatch. The opportunistic Selector
  /// poll in `drain_all` fires every
  /// `SELECTOR_POLL_INTERVAL` ticks instead of every
  /// tick — eliminates ~98% of the kernel transitions
  /// (kevent / epoll_wait) on the hot scheduling path.
  tick: Cell<u32>,
}

impl SchedulerState {
  const fn new() -> Self {
    Self {
      ctx: RefCell::new(Context::zeroed()),
      run_queue: RefCell::new(VecDeque::new()),
      current: Cell::new(None),
      selector: RefCell::new(None),
      poll_batch: RefCell::new(Vec::new()),
      tick: Cell::new(0),
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

  /// Run `f` with mutable access to this thread's
  /// [`Selector`], creating it on first use.
  ///
  /// Suspending FFIs (sockets, future stdin rework)
  /// reach the Selector through this entry point —
  /// the lazy-init keeps the scheduler thread-local
  /// const-constructed for cold start.
  pub(crate) fn with_selector_mut<R>(
    &self,
    f: impl FnOnce(&mut Selector) -> R,
  ) -> R {
    let mut guard = self.selector.borrow_mut();
    if guard.is_none() {
      *guard = Some(Selector::new());
    }
    // SAFETY: guard.is_none() handled immediately above.
    f(guard.as_mut().unwrap())
  }

  /// Whether any task is parked on the Selector for
  /// I/O readiness. False before the Selector is even
  /// created (no suspending FFI has run yet).
  pub(crate) fn selector_has_waiters(&self) -> bool {
    self
      .selector
      .borrow()
      .as_ref()
      .is_some_and(Selector::has_waiters)
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

/// Drains the run queue until every ready task has
/// finished. Called from non-task code (typically
/// `main` at a nursery scope's `}`) to run all
/// spawned siblings to completion before control
/// flows past the scope. Blocks in `Selector::poll(-1)`
/// when the run queue empties but I/O waiters remain;
/// returns only when both queues are empty.
pub fn drain_all() {
  // When called re-entrantly from inside a green task
  // (nested nursery), use non-blocking Selector poll
  // (timeout=0) instead of blocking poll(-1). Blocking
  // would freeze the only OS thread driving the
  // scheduler; any I/O waiter whose readiness depends
  // on the same thread would deadlock the program.
  let nested = with(|s| s.current().is_some());

  loop {
    let next = with(|s| s.pop_ready());

    match next {
      Some(task) => {
        // SAFETY: task pointer pulled from the run
        // queue is live (the box is scheduler-owned).
        unsafe { run_one(task) };

        // Opportunistic I/O drain — gated by tick
        // counter to avoid a kevent/epoll_wait
        // syscall on every dispatch.
        with(|s| {
          let tick = s.tick.get().wrapping_add(1);

          s.tick.set(tick);

          if tick % SELECTOR_POLL_INTERVAL == 0 {
            drain_ready(s, 0);
          }
        });
      }
      None => {
        let timeout = if nested { 0 } else { -1 };
        let woke = with(|s| drain_ready(s, timeout));

        if woke == 0 {
          return;
        }
      }
    }
  }
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
      Some(task) => {
        unsafe { run_one(task) };
        with(|s| drain_ready(s, 0));
      }
      None => {
        // Idle-poll before declaring deadlock — the
        // awaited task may be parked on I/O.
        let woke = with(|s| drain_ready(s, -1));
        if woke == 0 {
          // No one runnable AND no one parked on I/O,
          // yet the awaited task isn't Dead — genuine
          // deadlock. Loud abort beats a silent stall.
          panic!(
            "zo-scheduler deadlock: awaited task is Blocked \
             but run queue is empty",
          );
        }
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
/// Re-entrant: when called from inside a green task
/// (e.g. a nested `zo_nursery_drain` whose driver task
/// opens its own `nursery {}` to spawn children), we
/// snapshot `current` and the global scheduler-ctx slot
/// on the CPU stack and restore them once the inner
/// task yields / dies. Without this, the inner call's
/// `set_current(None)` strands the outer task with
/// `current = None` (its later `exit_current` then
/// panics), and the inner `ctx_switch` overwrites the
/// global ctx with a driver-stack resume point that
/// the outer task's eventual exit would walk to a
/// stale frame.
///
/// # Safety
///
/// `task` must be a valid `*mut ZoTask` with `state`
/// transitioning out of `Ready`.
unsafe fn run_one(task: *mut ZoTask) {
  // Every pthread that enters a green task must have
  // a signal-handler alternate stack installed — a
  // stack-overflow fault inside the task delivers the
  // signal onto the faulting (green) stack otherwise,
  // causing a double-fault. Idempotent per thread.
  crate::stack::ensure_sigaltstack();

  // SAFETY: caller contract.
  unsafe {
    (*task).state = TaskState::Running;
  }

  with(|s| {
    // Snapshot prior re-entrant state on the CPU stack
    // so a nested call doesn't strand the outer task.
    // `Context` is `#[repr(C)]` POD (gp regs + sp + lr),
    // ~168 B on aarch64 — a no-op compared to a green
    // task's 8 MiB stack reservation.
    let prev_current = s.current();
    let prev_sch_ctx: Context = unsafe { *s.scheduler_ctx_ptr() };

    s.set_current(Some(task));

    let sch_ctx = s.scheduler_ctx_ptr();
    // SAFETY: task pointer valid for the duration.
    let task_ctx = unsafe { &raw mut (*task).ctx };

    // Enter the task — returns when it yields / dies.
    unsafe {
      ctx_switch(sch_ctx, task_ctx);
    }

    // Restore the outer re-entrant state only when
    // nested (prev_current was Some). Top-level calls
    // leave sch_ctx as-is — the ctx_switch already
    // wrote a valid resume point into it, and
    // overwriting with the zeroed pre-switch snapshot
    // would leave a SIGSEGV landmine for any future
    // code that reads scheduler_ctx_ptr() before the
    // next ctx_switch stores a real LR/SP.
    if prev_current.is_some() {
      unsafe {
        *sch_ctx = prev_sch_ctx;
      }
    }

    s.set_current(prev_current);

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

/// Poll the Selector and re-queue every task whose fd
/// is now ready. Returns the number of tasks woken.
///
/// `timeout_ms`:
/// - `-1` — block in the kernel until any fd fires.
/// - ` 0` — non-blocking opportunistic drain.
/// - ` >0` — block up to that many ms.
///
/// Short-circuits to `0` if the Selector has no
/// waiters (avoids touching the lazy-init slot).
fn drain_ready(s: &SchedulerState, timeout_ms: c_int) -> usize {
  if !s.selector_has_waiters() {
    return 0;
  }

  let mut batch = s.poll_batch.borrow_mut();
  batch.clear();
  s.with_selector_mut(|sel| sel.poll(timeout_ms, &mut batch));

  let n = batch.len();

  for &task in batch.iter() {
    // SAFETY: task pointer was parked by a suspending
    // FFI on this scheduler thread; the box is live
    // until the task transitions to Dead.
    unsafe { (*task).state = TaskState::Ready };
    s.enqueue(task);
  }

  n
}

/// Non-blocking Selector poll — wake any I/O-parked
/// tasks whose fds are ready and enqueue them. Returns
/// the number of tasks woken. Exposed for `select.rs`'s
/// idle loop so it can surface I/O readiness without
/// blocking the OS thread.
pub fn poll_io_nonblocking() -> usize {
  with(|s| drain_ready(s, 0))
}

/// Cooperative park for non-task OS threads — drives the
/// scheduler while waiting on `park.notified`.
///
/// Today's `main` runs on the OS thread, not inside a
/// `ZoTask`. Without this helper, a blocking channel /
/// IO op called from `main` would freeze the only thread
/// driving the scheduler, the spawned task that would
/// produce the notification would never run, and the
/// program would silently deadlock. This is the
/// "first-five-minutes" trap: `imu x := rx.recv()`
/// inside `fun main` is the canonical Go shape and must
/// just work.
///
/// Loop body, in order:
/// 1. Fast path — if `notified` already latched, return.
/// 2. Pop the next ready green task and dispatch it via
///    [`run_one_external`]. The B1 re-entrant `run_one`
///    snapshots `current` + `sch_ctx` so nested wakes
///    don't strand anyone. After the task yields / dies,
///    opportunistically drain Selector readiness so any
///    I/O-parked task gets its turn before we loop.
/// 3. Local queue empty — non-blocking Selector poll. If
///    it woke anything, loop.
/// 4. Truly idle — block on `cv.wait_timeout` so we
///    re-check both `notified` and the Selector every
///    50 ms. A waker on another OS thread (future
///    multi-scheduler) or an I/O readiness event will
///    unblock us no later than the next tick.
pub(crate) fn park_thread_cooperative(park: &PthreadPark) {
  loop {
    // 1. Fast path: notification already arrived.
    {
      let guard = park.notified.lock().expect("PthreadPark poisoned");

      if *guard {
        return;
      }
    }

    // 2. Dispatch a local task if any.
    let next = with(|s| s.pop_ready());

    if let Some(task) = next {
      // SAFETY: pointer came from the run queue; the
      // owning `Box<ZoTask>` is live until the task
      // transitions to `Dead` and its handle is awaited.
      unsafe { run_one_external(task) };

      // Tick-gated I/O drain — same cadence as
      // drain_all to avoid syscall storms.
      with(|s| {
        let tick = s.tick.get().wrapping_add(1);

        s.tick.set(tick);

        if tick % SELECTOR_POLL_INTERVAL == 0 {
          drain_ready(s, 0);
        }
      });

      continue;
    }

    // 3. No local ready task — non-blocking I/O drain
    //    (always fires here since we're truly idle).
    let woke = with(|s| drain_ready(s, 0));

    if woke > 0 {
      continue;
    }

    // 4. Truly idle. Re-check `notified` under the lock
    //    to close the race where `unpark()` fired
    //    between the fast-path read and acquiring the
    //    lock, then wait with a short timeout so we
    //    keep polling the Selector for I/O wakes.
    let guard = park.notified.lock().expect("PthreadPark poisoned");

    if *guard {
      return;
    }

    let _ = park
      .cv
      .wait_timeout(guard, Duration::from_millis(50))
      .expect("PthreadPark wait poisoned");
  }
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
    // Drop the lazy-init Selector so a fresh kqueue /
    // epoll fd is opened for the next test — its
    // internal waiter map would otherwise carry stale
    // pointers from the previous test's task arena.
    *s.selector.borrow_mut() = None;
  });
}

// ===== Tests =====

#[cfg(test)]
mod tests {
  use super::*;
  use crate::task::{zo_task_await, zo_task_spawn_2};
  use std::os::unix::io::RawFd;

  /// Green task body: parks on `read_fd` via the
  /// Selector, yields with state `Blocked`, then reads
  /// one byte after the scheduler's idle-poll wakes it.
  extern "C-unwind" fn idle_poll_reader(read_fd: u64, _unused: u64) {
    let read_fd = read_fd as RawFd;

    // Park on the Selector.
    let me = with(|s| {
      let me = s.current().expect("reader has no task ctx");
      s.with_selector_mut(|sel| sel.register_read(read_fd, me));
      // SAFETY: `me` is the running task on this thread;
      // exclusive access is guaranteed by the cooperative
      // scheduler.
      unsafe {
        (*me).state = TaskState::Blocked;
      }
      me
    });
    let _ = me;

    // SAFETY: called from inside a green task body.
    unsafe { yield_now() };

    // Post-resume: the writer (OS thread) has injected
    // one byte. A non-blocking read returns it.
    let mut buf = [0u8; 1];
    let n = unsafe { libc::read(read_fd, buf.as_mut_ptr() as *mut _, 1) };
    assert_eq!(n, 1, "post-wake read returned {}", n);
    assert_eq!(buf[0], b'z');
  }

  #[test]
  fn idle_poll_wakes_task_from_external_thread() {
    reset_for_test();

    let mut fds = [0i32; 2];
    let rc = unsafe { libc::pipe(fds.as_mut_ptr()) };
    assert_eq!(rc, 0, "pipe() failed");
    let read_fd = fds[0];
    let write_fd = fds[1];

    // Non-blocking read end so a buggy wake path can't
    // stall the scheduler thread.
    unsafe {
      let flags = libc::fcntl(read_fd, libc::F_GETFL);
      libc::fcntl(read_fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
    }

    let reader =
      unsafe { zo_task_spawn_2(idle_poll_reader, read_fd as u64, 0) };

    // External OS thread injects the byte after a short
    // delay. The scheduler must block inside
    // `Selector::poll(-1)`, get woken by the kernel,
    // and re-queue the parked task.
    let write_fd_for_thread = write_fd;
    std::thread::spawn(move || {
      std::thread::sleep(std::time::Duration::from_millis(50));
      let byte = b"z";
      unsafe {
        libc::write(write_fd_for_thread, byte.as_ptr() as *const _, 1);
      }
    });

    // SAFETY: reader is a fresh `zo_task_spawn_2`
    // handle whose box outlives this call.
    unsafe { zo_task_await(reader) };

    unsafe {
      libc::close(read_fd);
      libc::close(write_fd);
    }
  }

  #[test]
  fn drain_all_returns_immediately_when_nothing_pending() {
    // No tasks, no selector waiters — drain_all is a
    // no-op. Guards against an idle-poll regression
    // where the new wiring would block in the kernel
    // even with nothing parked.
    reset_for_test();
    drain_all();
  }

  #[test]
  fn selector_has_waiters_reflects_registration() {
    reset_for_test();

    // Fresh scheduler — selector not yet allocated.
    assert!(!with(|s| s.selector_has_waiters()));

    let mut fds = [0i32; 2];
    unsafe { libc::pipe(fds.as_mut_ptr()) };
    let read_fd = fds[0];

    with(|s| {
      s.with_selector_mut(|sel| {
        sel.register_read(read_fd, 0xCAFE as *mut ZoTask)
      });
    });

    assert!(with(|s| s.selector_has_waiters()));

    unsafe {
      libc::close(fds[0]);
      libc::close(fds[1]);
    }

    // Cleanup so the sentinel pointer doesn't leak into
    // the next test's selector instance.
    reset_for_test();
  }
}
