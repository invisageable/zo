//! Multi-scheduler worker pool with work-stealing.
//!
//! A [`Pool`] launches `n` worker OS threads. Each owns
//! a cross-thread `SharedQueue` (Mutex-protected
//! `VecDeque`) that serves two purposes:
//!
//! - **Submission inbox** — [`Pool::spawn`] pushes into
//!   the emptiest worker's queue and wakes it.
//! - **Steal target** — when a worker finds both its
//!   shared queue AND its thread-local scheduler
//!   queue empty, it drains half of a neighbor's
//!   shared queue.
//!
//! A worker loops: shared queue → thread-local queue →
//! steal → park. The thread-local `SchedulerState` from
//! `scheduler.rs` still owns yield / wake / re-queue
//! semantics — a task that yields on worker-N gets
//! re-queued on worker-N's thread-local queue (stays
//! warm for cache locality). Stealing only relocates
//! *newly-spawned* work, not in-flight tasks.
//!
//! This primitive is **opt-in**. The default spawn /
//! await ABI (`zo_task_spawn` in `task.rs`) continues
//! to use a single thread-local scheduler — programs
//! that care about parallelism build a `Pool`
//! explicitly.
//!
//! Synchronization model: no per-task handles in this
//! minimal cut. Callers coordinate completion via their
//! own shared state (atomic counters, channels, etc.)
//! and call [`Pool::shutdown`] once that state says
//! "all work done". Per-task `.join()` is a follow-up.

use crate::scheduler;
use crate::task::ZoTask;

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

// ===== Types =====

/// `*mut ZoTask` wrapper that crosses the `Send`
/// boundary for stealing. Same pattern channels use
/// for their wait lists — the pointer is safe to ship
/// to another OS thread because only one worker holds
/// it at a time (FIFO pop from the shared queue).
#[derive(Copy, Clone)]
struct TaskPtr(*mut ZoTask);

// SAFETY: see type-level comment.
unsafe impl Send for TaskPtr {}

/// A worker's shared inbox. `Arc` + `Mutex` over a
/// `VecDeque`. Local ops are uncontended; stealing is
/// the only source of contention.
type SharedQueue = Arc<Mutex<VecDeque<TaskPtr>>>;

/// Parking primitive per worker. Flag-latched so an
/// `unpark()` before `park()` still releases.
struct WorkerPark {
  notified: Mutex<bool>,
  cv: Condvar,
}

impl WorkerPark {
  fn new() -> Self {
    Self {
      notified: Mutex::new(false),
      cv: Condvar::new(),
    }
  }

  fn unpark(&self) {
    let mut guard = self.notified.lock().expect("WorkerPark poisoned");

    *guard = true;

    self.cv.notify_one();
  }

  /// Park with a timeout so a stuck scheduler (e.g.
  /// all workers idle, no external signal) still gets
  /// a chance to observe the shutdown flag.
  fn park_with_timeout(&self, dur: Duration) {
    let mut guard = self.notified.lock().expect("WorkerPark poisoned");

    if !*guard {
      let (new_guard, _timeout) = self
        .cv
        .wait_timeout(guard, dur)
        .expect("WorkerPark poisoned");

      guard = new_guard;
    }

    // Clear the flag so the next park actually parks.
    *guard = false;
  }
}

// ===== Public API =====

/// A pool of worker threads running green tasks with
/// work-stealing between workers.
pub struct Pool {
  queues: Arc<Vec<SharedQueue>>,
  parks: Arc<Vec<Arc<WorkerPark>>>,
  shutdown_flag: Arc<AtomicBool>,
  /// Running count of submitted-but-not-yet-completed
  /// tasks. Bumped on `spawn`, decremented when a task
  /// reaches `Dead` inside `run_one_external`. Users
  /// can poll via [`Pool::pending`] to wait for
  /// quiescence.
  pending: Arc<AtomicUsize>,
  handles: Vec<JoinHandle<()>>,
}

impl Pool {
  /// Start a pool with `n` worker OS threads. Panics if
  /// `n == 0`.
  pub fn new(n: usize) -> Self {
    assert!(n > 0, "Pool::new requires n >= 1");

    let queues: Arc<Vec<SharedQueue>> = Arc::new(
      (0..n)
        .map(|_| Arc::new(Mutex::new(VecDeque::new())))
        .collect(),
    );

    let parks: Arc<Vec<Arc<WorkerPark>>> =
      Arc::new((0..n).map(|_| Arc::new(WorkerPark::new())).collect());

    let shutdown_flag = Arc::new(AtomicBool::new(false));
    let pending = Arc::new(AtomicUsize::new(0));

    let handles: Vec<JoinHandle<()>> = (0..n)
      .map(|my_id| {
        let queues = Arc::clone(&queues);
        let parks = Arc::clone(&parks);
        let shutdown = Arc::clone(&shutdown_flag);
        let pending = Arc::clone(&pending);

        thread::spawn(move || {
          worker_main(my_id, queues, parks, shutdown, pending)
        })
      })
      .collect();

    Self {
      queues,
      parks,
      shutdown_flag,
      pending,
      handles,
    }
  }

  /// Submit a task. Places it on the emptiest worker's
  /// shared queue and wakes that worker.
  pub fn spawn(&self, callee: extern "C-unwind" fn()) {
    let task = Box::into_raw(ZoTask::new_green_standalone(callee));

    self.pending.fetch_add(1, Ordering::SeqCst);

    let idx = self.emptiest_worker();

    self.queues[idx]
      .lock()
      .expect("pool queue poisoned")
      .push_back(TaskPtr(task));

    self.parks[idx].unpark();
  }

  /// How many tasks are still in flight (submitted but
  /// not yet Dead). Polling this to zero proves the
  /// pool has quiesced.
  pub fn pending(&self) -> usize {
    self.pending.load(Ordering::SeqCst)
  }

  /// Block the caller until every submitted task has
  /// completed. Spin-polls the pending counter with a
  /// small sleep — adequate for test / bench usage.
  pub fn wait_idle(&self) {
    while self.pending.load(Ordering::SeqCst) > 0 {
      thread::sleep(Duration::from_millis(1));
    }
  }

  /// Stop workers and join them. Any tasks still
  /// queued (never picked up) are reclaimed here so no
  /// `Box<ZoTask>` leaks even if a caller shuts down
  /// without [`wait_idle`](Self::wait_idle)-ing first.
  pub fn shutdown(mut self) {
    self.shutdown_flag.store(true, Ordering::SeqCst);

    for p in self.parks.iter() {
      p.unpark();
    }

    for h in self.handles.drain(..) {
      h.join().expect("pool worker joined dirty");
    }

    // Drain any queued-but-never-run tasks and reclaim
    // their stacks. A task still in a queue was never
    // entered, so dropping the Box is sufficient.
    for q in self.queues.iter() {
      for tp in q.lock().expect("pool queue poisoned").drain(..) {
        // SAFETY: task was queued but never run — no
        // waiters, no other references; pool owns the
        // Box exclusively.
        unsafe {
          drop(Box::from_raw(tp.0));
        }
      }
    }
  }

  /// Index of the worker whose shared queue is
  /// shortest. Ties broken by order (first queue wins).
  fn emptiest_worker(&self) -> usize {
    let mut best_idx = 0usize;
    let mut best_len = usize::MAX;

    for (i, q) in self.queues.iter().enumerate() {
      let len = q.lock().expect("pool queue poisoned").len();

      if len < best_len {
        best_len = len;
        best_idx = i;
      }
    }

    best_idx
  }
}

// ===== Worker loop =====

fn worker_main(
  my_id: usize,
  queues: Arc<Vec<SharedQueue>>,
  parks: Arc<Vec<Arc<WorkerPark>>>,
  shutdown: Arc<AtomicBool>,
  pending: Arc<AtomicUsize>,
) {
  let n = queues.len();

  loop {
    if shutdown.load(Ordering::SeqCst) {
      return;
    }

    // 1. Drain from my own shared queue — newly-spawned
    //    work + anything we stole last round.
    let next = queues[my_id]
      .lock()
      .expect("pool queue poisoned")
      .pop_front();

    if let Some(tp) = next {
      run_task(tp, &pending);
      continue;
    }

    // 2. Drain from the thread-local scheduler — where
    //    yield / channel-wake re-queues this worker's
    //    in-flight tasks.
    let local = scheduler::with(|s| s.pop_local());

    if let Some(task) = local {
      run_task(TaskPtr(task), &pending);
      continue;
    }

    // 3. Steal half from a neighbor. Round-robin
    //    victim selection keeps the probe cheap.
    if try_steal(my_id, n, &queues) {
      continue;
    }

    // 4. Nothing to do — park briefly. Timeout so we
    //    still observe the shutdown flag promptly
    //    without anyone unparking us.
    parks[my_id].park_with_timeout(Duration::from_millis(5));
  }
}

/// Drive one task to its next yield/die point.
/// Decrements `pending` once when the task transitions
/// to `Dead`, so external observers can poll for
/// quiescence. Reclaims the task's `Box<ZoTask>` on
/// Dead — fire-and-forget tasks don't leak.
fn run_task(tp: TaskPtr, pending: &Arc<AtomicUsize>) {
  // SAFETY: `tp.0` is a live `*mut ZoTask` pulled off a
  // pool queue. Only this worker holds it until it
  // re-enters the thread-local scheduler's state
  // machine via `run_one_external`.
  unsafe { scheduler::run_one_external(tp.0) };

  // SAFETY: same — task pointer still live; `state`
  // reflects the post-switch transition.
  let is_dead = unsafe { (*tp.0).state == crate::task::TaskState::Dead };

  if is_dead {
    pending.fetch_sub(1, Ordering::SeqCst);

    // SAFETY: task is Dead — no outstanding references
    // (waiters were flushed in `exit_current`, the pool
    // is the last owner of the handle). Reclaim the
    // Box<ZoTask> + its 256 KB stack.
    unsafe {
      drop(Box::from_raw(tp.0));
    }
  }
}

/// Try stealing half of a neighbor's shared queue.
/// Returns `true` if any work was stolen.
fn try_steal(my_id: usize, n: usize, queues: &[SharedQueue]) -> bool {
  for offset in 1..n {
    let victim = (my_id + offset) % n;

    let mut vq = queues[victim].lock().expect("pool queue poisoned");
    let half = vq.len().div_ceil(2);

    if half == 0 {
      continue;
    }

    let stolen: Vec<TaskPtr> = vq.drain(..half).collect();

    drop(vq);

    let mut mine = queues[my_id].lock().expect("pool queue poisoned");

    for tp in stolen {
      mine.push_back(tp);
    }

    return true;
  }

  false
}

// ===== Tests =====

// ===== FFI exports for core/thread.zo =====

/// Create a pool with `n` worker threads. Returns an
/// opaque handle (leaked `Box<Pool>` pointer).
///
/// # Safety
///
/// No preconditions.
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn zo_pool_new(n: i32) -> i64 {
  let workers = if n > 0 { n as usize } else { 1 };
  let pool = Box::new(Pool::new(workers));

  Box::into_raw(pool) as i64
}

/// Submit a zero-arg function to the pool.
///
/// # Safety
///
/// `handle` must be a live pool from `zo_pool_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn zo_pool_spawn(
  handle: i64,
  callee: extern "C-unwind" fn(),
) {
  let pool = unsafe { &*(handle as *const Pool) };

  pool.spawn(callee);
}

/// Block until all submitted tasks complete.
///
/// # Safety
///
/// `handle` must be a live pool from `zo_pool_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn zo_pool_wait_idle(handle: i64) {
  let pool = unsafe { &*(handle as *const Pool) };

  pool.wait_idle();
}

/// Shutdown the pool — drain queues and join workers.
/// Consumes the handle.
///
/// # Safety
///
/// `handle` must be a live pool (or 0 for no-op).
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn zo_pool_shutdown(handle: i64) {
  if handle != 0 {
    let pool = unsafe { Box::from_raw(handle as *mut Pool) };

    pool.shutdown();
  }
}

/// Number of worker threads in the pool.
///
/// # Safety
///
/// `handle` must be a live pool from `zo_pool_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn zo_pool_worker_count(handle: i64) -> i32 {
  let pool = unsafe { &*(handle as *const Pool) };

  pool.queues.len() as i32
}

#[cfg(test)]
mod tests {
  use super::*;

  use std::sync::atomic::AtomicU32;

  // Each test owns its own `static` counter + its
  // own `extern "C-unwind" fn` — the calling
  // convention forbids captures, so the only way to
  // observe a task's side effect is through a fixed-
  // address location. One counter per test keeps the
  // tests independent so they can run concurrently
  // under `cargo test` without racing on shared
  // state.

  #[test]
  fn pool_runs_submitted_tasks_across_workers() {
    static COUNTER: AtomicU32 = AtomicU32::new(0);

    extern "C-unwind" fn inc() {
      COUNTER.fetch_add(1, Ordering::SeqCst);
    }

    COUNTER.store(0, Ordering::SeqCst);

    let pool = Pool::new(4);

    for _ in 0..1_000 {
      pool.spawn(inc);
    }

    pool.wait_idle();

    assert_eq!(COUNTER.load(Ordering::SeqCst), 1_000);
    assert_eq!(pool.pending(), 0);

    pool.shutdown();
  }

  #[test]
  fn pool_with_single_worker_still_runs() {
    // Edge case: a pool with exactly one worker
    // degrades to a single-threaded scheduler. No
    // stealing possible. Validates the steal-skip path.
    static COUNTER: AtomicU32 = AtomicU32::new(0);

    extern "C-unwind" fn inc() {
      COUNTER.fetch_add(1, Ordering::SeqCst);
    }

    COUNTER.store(0, Ordering::SeqCst);

    let pool = Pool::new(1);

    for _ in 0..100 {
      pool.spawn(inc);
    }

    pool.wait_idle();

    assert_eq!(COUNTER.load(Ordering::SeqCst), 100);

    pool.shutdown();
  }

  #[test]
  fn pool_work_stealing_balances_load() {
    // Submit all work before workers get a chance to
    // pick up — stealing should rebalance. With 10k
    // tasks and 8 workers, the initial "emptiest
    // queue" heuristic still produces skew; stealing
    // must smooth it out.
    static COUNTER: AtomicU32 = AtomicU32::new(0);

    extern "C-unwind" fn inc() {
      COUNTER.fetch_add(1, Ordering::SeqCst);
    }

    COUNTER.store(0, Ordering::SeqCst);

    let pool = Pool::new(8);

    for _ in 0..10_000 {
      pool.spawn(inc);
    }

    pool.wait_idle();

    assert_eq!(COUNTER.load(Ordering::SeqCst), 10_000);

    pool.shutdown();
  }
}
