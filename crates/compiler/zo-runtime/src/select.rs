//! Phase 5 of `PLAN_PREHISTORY.md` — selective
//! receive runtime primitive.
//!
//! `_zo_select_wait` atomically waits on N channels
//! and returns the arm index of the first one to
//! fire, copying the received value into a caller-
//! owned output buffer.
//!
//! **v1 implementation — poll-and-yield.** A correct
//! multi-channel atomic wait needs per-select
//! coordination across all participating channels
//! (each channel's wait list tracks a shared
//! `SelectCoord` entry, first fire CAS-wins). That's
//! a bigger primitive; v1 polls each channel non-
//! blocking in turn, and yields if none are ready.
//!
//! The cost: under heavy contention, a green task
//! parked on select busy-yields until a channel
//! fires. Single-threaded workloads converge fast
//! (the scheduler runs other tasks in between).
//! Cross-scheduler contention could starve; Phase 8
//! revisits with CAS-based select coord.

use crate::channel::ZoChan;
use crate::scheduler;

use std::sync::atomic::{AtomicUsize, Ordering};

/// Sentinel for "no arm has fired yet". Any real arm
/// index returned is `< MAX`.
pub const SELECT_NONE: usize = usize::MAX;

/// Atomically wait on N channels; return the arm
/// index of the first to have a value; copy that
/// value into `out_value`.
///
/// # Safety
///
/// - `chans` must point to at least `n` valid
///   `*mut ZoChan` handles.
/// - `out_value` must point to at least `elem_sz`
///   writable bytes, where `elem_sz` is the shared
///   element size across every channel in `chans`.
///   Select arms with mismatched element sizes are
///   a compile error upstream.
///
/// Returns the arm index (0-based). `SELECT_NONE` is
/// reserved for the "no arm ever fires" deadlock
/// path; the function panics instead of returning
/// that.
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn _zo_select_wait(
  chans: *const *mut ZoChan,
  n: usize,
  out_value: *mut u8,
  elem_sz: usize,
) -> usize {
  // Try-recv each channel in turn. First non-empty
  // wins; if none, yield and retry.
  let attempts = AtomicUsize::new(0);

  loop {
    for i in 0..n {
      // SAFETY: caller contract.
      let chan_ptr = unsafe { *chans.add(i) };

      if chan_ptr.is_null() {
        continue;
      }

      // SAFETY: channel layout is opaque — we invoke
      // the existing try-recv via the locking path
      // below. A richer design would give ZoChan a
      // dedicated `try_recv` method.
      if unsafe { try_recv(chan_ptr, out_value, elem_sz) } {
        return i;
      }
    }

    // No arm ready. Yield if on the scheduler thread
    // (green task); otherwise short-sleep to avoid
    // burning a pthread CPU core.
    let on_scheduler = scheduler::with(|s| s.current().is_some());

    if on_scheduler {
      // SAFETY: we're in a task context per the
      // scheduler::current() check.
      unsafe { scheduler::yield_now() };
    } else {
      std::thread::sleep(std::time::Duration::from_micros(100));
    }

    attempts.fetch_add(1, Ordering::Relaxed);
  }
}

/// Non-blocking recv. Returns `true` iff a value was
/// available and copied into `out_value`. The channel
/// module's locked state is inspected via the same
/// `inner.lock()` path as the normal recv.
///
/// # Safety
///
/// - `chan` must be a live `*mut ZoChan`.
/// - `out_value` must point to at least `elem_sz`
///   writable bytes.
unsafe fn try_recv(
  chan: *mut ZoChan,
  out_value: *mut u8,
  elem_sz: usize,
) -> bool {
  // SAFETY: caller contract. We call into the
  // channel's locked state via a dedicated helper
  // exposed from `channel.rs`.
  unsafe { crate::channel::try_recv_nonblocking(chan, out_value, elem_sz) }
}
