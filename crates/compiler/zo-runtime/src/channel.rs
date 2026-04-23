//! Channel runtime with scheduler-integrated parking.
//!
//! A channel is a mutex-protected FIFO queue with two
//! wait lists (senders blocked on "buffer full",
//! receivers blocked on "buffer empty"). Parking is
//! polymorphic:
//!
//! - **Green task caller** — `scheduler::current()` is
//!   `Some`. The task enqueues itself on the wait list
//!   as `Waiter::Green(task)`, marks Blocked, yields
//!   via [`scheduler::yield_now`]. When the matching op
//!   pops it, the waker sets `Ready` + pushes to the
//!   run queue.
//! - **Non-task caller** — main thread before any
//!   spawn, or a `std::thread::spawn`ed helper running
//!   alongside the scheduler. Parks on a per-wait
//!   `Arc<PthreadPark>` (Condvar + notified flag).
//!   Waker calls `unpark()`; the OS thread resumes.
//!
//! Hybrid parking lets pthread helpers exchange values
//! with main over a rendezvous channel (the simple
//! blocking case) AND lets a green task park on
//! send / recv and be woken by another green task on
//! the same scheduler.
//!
//! Cross-OS-thread wake of a parked green task (e.g.
//! `std::thread::spawn`ed helper sending to a channel
//! whose receiver is a parked green task on the
//! scheduler thread) is not supported — it would need
//! a cross-scheduler wake primitive beyond the single-
//! scheduler green-task model.
//!
//! The `#[no_mangle] extern "C-unwind"` exports carry
//! the ABI that the ARM codegen's `BL _zo_chan_*`
//! placeholders resolve against.

use crate::scheduler;
use crate::task::{TaskState, ZoTask};

use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex};

/// Per-channel runtime state. Heap-allocated; zo code
/// holds only the opaque `*mut ZoChan` handle.
pub struct ZoChan {
  inner: Mutex<ChannelInner>,
  elem_sz: usize,
  capacity: usize,
}

/// Data protected by the channel's mutex.
struct ChannelInner {
  /// FIFO of raw byte buffers, one per in-flight value.
  queue: VecDeque<Vec<u8>>,
  /// True when a caller has closed the channel. Send
  /// on a closed channel panics; recv drains any
  /// buffered values and then zero-fills.
  closed: bool,
  /// Senders parked on "buffer full".
  senders: VecDeque<Waiter>,
  /// Receivers parked on "buffer empty".
  receivers: VecDeque<Waiter>,
}

/// A parked caller on a channel wait list.
#[derive(Clone)]
enum Waiter {
  /// Green task parked on the scheduler. On wake,
  /// `state` transitions to `Ready` and the task is
  /// re-enqueued.
  Green(TaskPtr),
  /// Non-task OS thread parked on a personal Condvar.
  /// On wake, the Condvar is signalled and the thread
  /// resumes.
  Pthread(Arc<PthreadPark>),
}

/// `*mut ZoTask` wrapper giving it `Send` — needed so
/// `Waiter` can live in `Mutex<ChannelInner>` which
/// requires `Send + Sync` for cross-thread access.
///
/// # Safety
///
/// Only one OS thread (the scheduler thread)
/// dereferences the pointer at a time — the single-
/// owner invariant. A multi-scheduler design would
/// either keep this invariant (ship-the-box) or swap
/// to task-ID lookup.
#[derive(Copy, Clone)]
struct TaskPtr(*mut ZoTask);

unsafe impl Send for TaskPtr {}

/// Per-wait parking primitive for non-task callers.
/// Pair of `Mutex<bool>` + `Condvar` implements a
/// one-shot binary semaphore — wake-up is latched, so
/// `unpark` before `park` still releases the parker.
struct PthreadPark {
  /// `true` once `unpark()` has been called.
  notified: Mutex<bool>,
  cv: Condvar,
}

impl PthreadPark {
  fn new() -> Self {
    Self {
      notified: Mutex::new(false),
      cv: Condvar::new(),
    }
  }

  /// Block the OS thread until a matching `unpark()`.
  /// If `unpark()` already fired, returns immediately.
  fn park(&self) {
    let mut guard = self.notified.lock().expect("PthreadPark poisoned");

    while !*guard {
      guard = self.cv.wait(guard).expect("PthreadPark wait poisoned");
    }
  }

  /// Wake the parked thread. Latches the flag so a
  /// subsequent `park()` returns immediately.
  fn unpark(&self) {
    let mut guard = self.notified.lock().expect("PthreadPark poisoned");

    *guard = true;

    self.cv.notify_one();
  }

  /// Park for at most `timeout`. Returns `true` if
  /// `unpark` latched, `false` on timeout. Used by
  /// `_zo_chan_recv_timeout`.
  fn park_timed(&self, timeout: std::time::Duration) -> bool {
    let mut guard = self.notified.lock().expect("PthreadPark poisoned");

    if *guard {
      return true;
    }

    let (new_guard, wait_result) = self
      .cv
      .wait_timeout(guard, timeout)
      .expect("PthreadPark wait poisoned");

    guard = new_guard;

    if wait_result.timed_out() && !*guard {
      return false;
    }

    *guard
  }
}

/// A parked-caller handle returned by
/// [`park_and_register`]. The caller invokes `wait()`
/// AFTER dropping the channel's mutex, so the matching
/// op can wake them without contending on the lock.
enum ParkHandle {
  Green(TaskPtr),
  Pthread(Arc<PthreadPark>),
}

impl ParkHandle {
  /// Block the caller. Must be invoked after the
  /// caller has dropped the channel's `inner` mutex.
  fn wait(self) {
    match self {
      // SAFETY: task ptr is still live — either we're
      // still executing on its stack (same-scheduler
      // park) or the scheduler owns the Box.
      Self::Green(task) => unsafe {
        (*task.0).state = TaskState::Blocked;

        scheduler::yield_now();
      },
      Self::Pthread(p) => p.park(),
    }
  }
}

/// Register the current caller on `waitlist`. Returns
/// a [`ParkHandle`] the caller drops the channel's
/// mutex first, then invokes `wait()` on. Must be
/// called with the channel's `inner` mutex held so
/// the waiter registration + wait-for-wake pair is
/// atomic w.r.t. the matching counterpart.
fn park_and_register(waitlist: &mut VecDeque<Waiter>) -> ParkHandle {
  match scheduler::with(|s| s.current()) {
    Some(task) => {
      let ptr = TaskPtr(task);

      waitlist.push_back(Waiter::Green(ptr));

      ParkHandle::Green(ptr)
    }
    None => {
      let p = Arc::new(PthreadPark::new());

      waitlist.push_back(Waiter::Pthread(Arc::clone(&p)));

      ParkHandle::Pthread(p)
    }
  }
}

/// Wake a previously-parked waiter. MUST be called
/// after the channel's `inner` mutex has been
/// dropped — the green-task wake path re-enters the
/// scheduler to push to the run queue, and holding
/// the channel mutex across that would force later
/// same-thread channel ops to block.
fn wake(w: Waiter) {
  match w {
    // SAFETY: task ptr is valid while the green task
    // remains in the Ready / Blocked states — the
    // scheduler owns the Box via its `current` +
    // `run_queue` references.
    Waiter::Green(task) => unsafe {
      (*task.0).state = TaskState::Ready;

      scheduler::with(|s| s.enqueue(task.0));
    },
    Waiter::Pthread(p) => p.unpark(),
  }
}

impl ZoChan {
  fn new(elem_sz: usize, capacity: usize) -> Self {
    Self {
      inner: Mutex::new(ChannelInner {
        queue: VecDeque::with_capacity(capacity.max(1)),
        closed: false,
        senders: VecDeque::new(),
        receivers: VecDeque::new(),
      }),
      elem_sz,
      capacity,
    }
  }
}

// ===== C ABI exports =====

/// Allocate a fresh channel.
///
/// # Safety
///
/// The returned pointer must be released via
/// [`_zo_chan_free`]. Cross-thread sharing is safe —
/// `ZoChan` is `Send + Sync`.
#[unsafe(no_mangle)]
pub extern "C-unwind" fn _zo_chan_new(
  elem_sz: usize,
  capacity: usize,
) -> *mut ZoChan {
  Box::into_raw(Box::new(ZoChan::new(elem_sz, capacity)))
}

/// Push a value. Parks the caller via the scheduler
/// (green task) or a Condvar (non-task) when the
/// bounded buffer is full.
///
/// # Safety
///
/// - `chan` must come from [`_zo_chan_new`] and still
///   be live.
/// - `src` must point to at least `elem_sz` bytes of
///   readable memory laid out exactly as the compiler
///   declared the channel's element type.
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn _zo_chan_send(
  chan: *mut ZoChan,
  src: *const u8,
) {
  // SAFETY: caller contract.
  let ch = unsafe { &*chan };
  let bound = ch.capacity.max(1);

  loop {
    let mut guard = ch.inner.lock().expect("zo-chan poisoned");

    if guard.closed {
      panic!("send on closed zo-chan");
    }

    if guard.queue.len() < bound {
      // SAFETY: `src..src + elem_sz` is valid to
      // read per caller contract; `elem_sz` matches
      // what this channel was constructed with.
      let mut buf = vec![0u8; ch.elem_sz];

      unsafe {
        std::ptr::copy_nonoverlapping(src, buf.as_mut_ptr(), ch.elem_sz);
      }

      guard.queue.push_back(buf);

      // Wake a matching receiver if one is parked. We
      // pop under the lock + wake outside it — see the
      // comment on `wake` for why.
      let waker = guard.receivers.pop_front();

      drop(guard);

      if let Some(w) = waker {
        wake(w);
      }

      return;
    }

    // Buffer full — park.
    let handle = park_and_register(&mut guard.senders);

    drop(guard);

    handle.wait();

    // Woken — loop back and retry. Another sender may
    // have refilled the buffer between wake and
    // re-lock, so we re-check rather than assume
    // room.
  }
}

/// Pop a value. Parks the caller via the scheduler
/// (green task) or a Condvar (non-task) when the
/// channel is empty.
///
/// # Safety
///
/// - `chan` must come from [`_zo_chan_new`] and still
///   be live.
/// - `dst` must point to at least `elem_sz` writable
///   bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn _zo_chan_recv(chan: *mut ZoChan, dst: *mut u8) {
  // SAFETY: caller contract.
  let ch = unsafe { &*chan };

  loop {
    let mut guard = ch.inner.lock().expect("zo-chan poisoned");

    if let Some(buf) = guard.queue.pop_front() {
      // SAFETY: `buf.len() == elem_sz` by construction
      // in `_zo_chan_send`; `dst` writable per caller.
      unsafe {
        std::ptr::copy_nonoverlapping(buf.as_ptr(), dst, ch.elem_sz);
      }

      let waker = guard.senders.pop_front();

      drop(guard);

      if let Some(w) = waker {
        wake(w);
      }

      return;
    }

    if guard.closed {
      // Zero-fill on closed drain — richer Option
      // return is deferred until the cancel wiring
      // lands.
      unsafe {
        std::ptr::write_bytes(dst, 0, ch.elem_sz);
      }

      return;
    }

    // Empty — park.
    let handle = park_and_register(&mut guard.receivers);

    drop(guard);

    handle.wait();
  }
}

/// Non-blocking recv for use by the `select`
/// primitive (see `select.rs`). Returns `true` iff a
/// value was available and copied into `dst`.
/// Unlike `_zo_chan_recv`, this function never parks
/// the caller — if the channel is empty it returns
/// `false` immediately.
///
/// When a value IS popped, this wakes one parked
/// sender (same policy as `_zo_chan_recv`) so the
/// channel keeps flowing.
///
/// # Safety
///
/// - `chan` must be a live `*mut ZoChan`.
/// - `dst` must point to at least `elem_sz` writable
///   bytes matching the channel's element size.
pub unsafe fn try_recv_nonblocking(
  chan: *mut ZoChan,
  dst: *mut u8,
  elem_sz: usize,
) -> bool {
  // SAFETY: caller contract.
  let ch = unsafe { &*chan };

  debug_assert_eq!(
    ch.elem_sz, elem_sz,
    "select arm's elem_sz must match the channel's",
  );

  let mut guard = ch.inner.lock().expect("zo-chan poisoned");

  let buf = match guard.queue.pop_front() {
    Some(b) => b,
    None => return false,
  };

  // SAFETY: `buf.len() == elem_sz` by construction
  // in `_zo_chan_send`; dst writable per caller.
  unsafe {
    std::ptr::copy_nonoverlapping(buf.as_ptr(), dst, ch.elem_sz);
  }

  let waker = guard.senders.pop_front();

  drop(guard);

  if let Some(w) = waker {
    wake(w);
  }

  true
}

/// Close a channel. Wakes every parked sender and
/// receiver so they observe the closed state and
/// unwind. After close:
///
/// - `_zo_chan_send` on the channel panics.
/// - `_zo_chan_recv` drains any buffered values; once
///   the buffer is empty it zero-fills `dst` and
///   returns immediately.
///
/// Idempotent — close-on-closed is a no-op.
///
/// # Safety
///
/// `chan` must be a live pointer from [`_zo_chan_new`].
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn _zo_chan_close(chan: *mut ZoChan) {
  // SAFETY: caller contract.
  let ch = unsafe { &*chan };

  let mut guard = ch.inner.lock().expect("zo-chan poisoned");

  if guard.closed {
    return;
  }

  guard.closed = true;

  // Move every waiter out under the lock; wake outside.
  let senders: Vec<Waiter> = guard.senders.drain(..).collect();
  let receivers: Vec<Waiter> = guard.receivers.drain(..).collect();

  drop(guard);

  for w in senders {
    wake(w);
  }

  for w in receivers {
    wake(w);
  }
}

/// Timed recv. Returns `true` iff a value was received
/// within `timeout_ms`; `false` on timeout. Zero-fills
/// `dst` on timeout so the caller doesn't observe
/// uninitialized memory.
///
/// Non-task callers only — green tasks fall back to
/// plain blocking recv (timeout ignored). Green-task
/// timed recv would need a scheduler-integrated timer
/// wheel.
///
/// # Safety
///
/// - `chan` must be a live pointer from [`_zo_chan_new`].
/// - `dst` must point to at least `elem_sz` writable
///   bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn _zo_chan_recv_timeout(
  chan: *mut ZoChan,
  dst: *mut u8,
  timeout_ms: u64,
) -> bool {
  // SAFETY: caller contract.
  let ch = unsafe { &*chan };
  let deadline =
    std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);

  loop {
    let mut guard = ch.inner.lock().expect("zo-chan poisoned");

    if let Some(buf) = guard.queue.pop_front() {
      // SAFETY: matches `_zo_chan_recv` path.
      unsafe {
        std::ptr::copy_nonoverlapping(buf.as_ptr(), dst, ch.elem_sz);
      }

      let waker = guard.senders.pop_front();

      drop(guard);

      if let Some(w) = waker {
        wake(w);
      }

      return true;
    }

    if guard.closed {
      // SAFETY: `dst` writable per caller.
      unsafe {
        std::ptr::write_bytes(dst, 0, ch.elem_sz);
      }

      return false;
    }

    let now = std::time::Instant::now();

    if now >= deadline {
      unsafe {
        std::ptr::write_bytes(dst, 0, ch.elem_sz);
      }

      return false;
    }

    // Green tasks don't participate in timed wait in
    // v1 — they fall back to plain park. Non-task
    // callers park on a Condvar with timeout.
    if scheduler::with(|s| s.current()).is_some() {
      let handle = park_and_register(&mut guard.receivers);

      drop(guard);

      handle.wait();
    } else {
      let park = std::sync::Arc::new(PthreadPark::new());

      guard
        .receivers
        .push_back(Waiter::Pthread(std::sync::Arc::clone(&park)));

      drop(guard);

      let remaining = deadline.saturating_duration_since(now);
      let waited = park.park_timed(remaining);

      if !waited {
        // Timed out — remove our stale waiter entry so
        // no future sender tries to wake a dropped
        // parker. Searching is O(n) in the waiter list;
        // acceptable for a v1 timeout path.
        let mut guard = ch.inner.lock().expect("zo-chan poisoned");

        guard.receivers.retain(|w| match w {
          Waiter::Pthread(p) => !std::sync::Arc::ptr_eq(p, &park),
          Waiter::Green(_) => true,
        });

        drop(guard);

        unsafe {
          std::ptr::write_bytes(dst, 0, ch.elem_sz);
        }

        return false;
      }
    }
  }
}

/// Release a channel.
///
/// # Safety
///
/// `chan` must have come from [`_zo_chan_new`] and
/// must not be used after this call returns.
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn _zo_chan_free(chan: *mut ZoChan) {
  if chan.is_null() {
    return;
  }

  // SAFETY: exclusive ownership transfers on drop.
  drop(unsafe { Box::from_raw(chan) });
}

// ===== Tests =====

#[cfg(test)]
mod tests {
  use super::*;

  use crate::task::{_zo_task_await, _zo_task_spawn};

  use std::sync::atomic::{AtomicU64, Ordering};

  // ----- pure pthread-context tests -----
  //
  // No green tasks involved. These exercise the
  // `Waiter::Pthread` path — main thread or
  // `std::thread::spawn`ed helpers exchanging values
  // without the scheduler in the loop.

  #[test]
  fn send_recv_round_trips_a_u64() {
    unsafe {
      let ch = _zo_chan_new(std::mem::size_of::<u64>(), 4);
      let src: u64 = 0xDEAD_BEEF_CAFE_BABE;
      let mut dst: u64 = 0;

      _zo_chan_send(ch, (&raw const src).cast::<u8>());
      _zo_chan_recv(ch, (&raw mut dst).cast::<u8>());

      assert_eq!(dst, src);

      _zo_chan_free(ch);
    }
  }

  #[test]
  fn buffered_fifo_order_is_preserved() {
    unsafe {
      let ch = _zo_chan_new(std::mem::size_of::<u32>(), 4);

      for v in [1u32, 2, 3, 4] {
        _zo_chan_send(ch, (&raw const v).cast::<u8>());
      }

      let mut out = [0u32; 4];

      for slot in out.iter_mut() {
        _zo_chan_recv(ch, (slot as *mut u32).cast::<u8>());
      }

      assert_eq!(out, [1, 2, 3, 4]);

      _zo_chan_free(ch);
    }
  }

  #[test]
  fn unbuffered_rendezvous_matches_sender_to_receiver() {
    use std::thread;

    unsafe {
      let ch = _zo_chan_new(std::mem::size_of::<u64>(), 0);

      // `ch` is a raw pointer, not Send; wrap in usize
      // to ferry it across the thread boundary.
      let ch_addr = ch as usize;

      let producer = thread::spawn(move || {
        let v: u64 = 42;
        let ch = ch_addr as *mut ZoChan;

        _zo_chan_send(ch, (&raw const v).cast::<u8>());
      });

      let mut out: u64 = 0;

      _zo_chan_recv(ch, (&raw mut out).cast::<u8>());
      producer.join().unwrap();

      assert_eq!(out, 42);

      _zo_chan_free(ch);
    }
  }

  // ----- green-task parking tests -----

  // Channel shared between green tasks + the main
  // thread's setup. `AtomicU64` stores the raw pointer
  // so the two `extern "C-unwind"` entries can grab it
  // without captures (forbidden across that ABI).
  static SHARED_CH: AtomicU64 = AtomicU64::new(0);
  static RECEIVED: AtomicU64 = AtomicU64::new(0);

  extern "C-unwind" fn green_sender() {
    let ch = SHARED_CH.load(Ordering::SeqCst) as *mut ZoChan;
    let v: u64 = 0x1234;

    unsafe {
      _zo_chan_send(ch, (&raw const v).cast::<u8>());
    }
  }

  extern "C-unwind" fn green_receiver() {
    let ch = SHARED_CH.load(Ordering::SeqCst) as *mut ZoChan;
    let mut v: u64 = 0;

    unsafe {
      _zo_chan_recv(ch, (&raw mut v).cast::<u8>());
    }

    RECEIVED.store(v, Ordering::SeqCst);
  }

  #[test]
  fn two_green_tasks_exchange_via_buffered_channel() {
    // Capacity 1 — sender + receiver shouldn't block
    // each other. Proves green-task parking doesn't
    // regress same-scheduler non-contended cases.
    scheduler::reset_for_test();

    unsafe {
      let ch = _zo_chan_new(std::mem::size_of::<u64>(), 1);

      SHARED_CH.store(ch as u64, Ordering::SeqCst);
      RECEIVED.store(0, Ordering::SeqCst);

      let sender = _zo_task_spawn(green_sender);
      let receiver = _zo_task_spawn(green_receiver);

      _zo_task_await(sender);
      _zo_task_await(receiver);

      _zo_chan_free(ch);
    }

    assert_eq!(RECEIVED.load(Ordering::SeqCst), 0x1234);
  }

  // ----- close + timeout tests -----

  #[test]
  fn close_wakes_parked_receiver_returns_zero() {
    use std::thread;

    unsafe {
      let ch = _zo_chan_new(std::mem::size_of::<u64>(), 0);
      let ch_addr = ch as usize;

      // Receiver parks on empty channel.
      let recver = thread::spawn(move || {
        let ch = ch_addr as *mut ZoChan;
        let mut out: u64 = 0xDEAD_BEEF;
        _zo_chan_recv(ch, (&raw mut out).cast::<u8>());

        out
      });

      // Give the receiver time to park.
      thread::sleep(std::time::Duration::from_millis(30));

      _zo_chan_close(ch);

      let out = recver.join().unwrap();

      // Closed empty channel zero-fills.
      assert_eq!(out, 0);

      _zo_chan_free(ch);
    }
  }

  #[test]
  fn close_idempotent() {
    unsafe {
      let ch = _zo_chan_new(std::mem::size_of::<u32>(), 0);

      _zo_chan_close(ch);
      _zo_chan_close(ch);
      _zo_chan_close(ch);

      _zo_chan_free(ch);
    }
  }

  #[test]
  fn recv_timeout_fires_on_empty_channel() {
    unsafe {
      let ch = _zo_chan_new(std::mem::size_of::<u64>(), 0);
      let mut out: u64 = 0xFFFF_FFFF_FFFF_FFFF;

      let got = _zo_chan_recv_timeout(ch, (&raw mut out).cast::<u8>(), 20);

      assert!(!got, "recv_timeout should return false on timeout");
      assert_eq!(out, 0, "dst zero-filled on timeout");

      _zo_chan_free(ch);
    }
  }

  #[test]
  fn recv_timeout_returns_true_when_value_available() {
    unsafe {
      let ch = _zo_chan_new(std::mem::size_of::<u64>(), 1);
      let src: u64 = 0xABCD_EF01;

      _zo_chan_send(ch, (&raw const src).cast::<u8>());

      let mut out: u64 = 0;
      let got = _zo_chan_recv_timeout(ch, (&raw mut out).cast::<u8>(), 100);

      assert!(got);
      assert_eq!(out, src);

      _zo_chan_free(ch);
    }
  }

  #[test]
  #[should_panic(expected = "send on closed zo-chan")]
  fn send_on_closed_channel_panics() {
    unsafe {
      let ch = _zo_chan_new(std::mem::size_of::<u32>(), 4);

      _zo_chan_close(ch);

      let src: u32 = 42;
      _zo_chan_send(ch, (&raw const src).cast::<u8>());

      _zo_chan_free(ch);
    }
  }

  #[test]
  fn green_receiver_parks_then_resumes_on_send() {
    // Capacity 0 — receiver MUST park before any value
    // is available. Proves the green-task park-on-empty
    // + wake-on-send path.
    scheduler::reset_for_test();

    unsafe {
      let ch = _zo_chan_new(std::mem::size_of::<u64>(), 0);

      SHARED_CH.store(ch as u64, Ordering::SeqCst);
      RECEIVED.store(0, Ordering::SeqCst);

      // Spawn receiver FIRST so it blocks on empty.
      let receiver = _zo_task_spawn(green_receiver);

      // Spawn sender SECOND; when it runs, it'll wake
      // the parked receiver.
      let sender = _zo_task_spawn(green_sender);

      _zo_task_await(receiver);
      _zo_task_await(sender);

      _zo_chan_free(ch);
    }

    assert_eq!(RECEIVED.load(Ordering::SeqCst), 0x1234);
  }
}
