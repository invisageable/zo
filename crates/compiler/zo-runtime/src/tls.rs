//! Task-local storage.
//!
//! Per-green-task key/value map, addressed by a u64
//! key and carrying opaque byte-sized payloads.
//! Lookup goes through `scheduler::current()`: when
//! called from a green task the TLS slot lives on
//! the task; when called from a non-task OS thread
//! it falls back to a thread-local HashMap so
//! main-thread code and `std::thread::spawn`ed
//! helpers can still use the API.
//!
//! Runtime exports:
//! - `_zo_tls_set(key, src, elem_sz)` — store bytes.
//! - `_zo_tls_get(key, dst, elem_sz) -> bool` —
//!   copy bytes out; returns false if the key isn't
//!   set.
//!
//! The storage shape (byte-vector values keyed by
//! u64) keeps the runtime type-agnostic — the
//! compiler owns layout, the runtime only moves
//! bytes.

use std::cell::RefCell;
use std::collections::HashMap;

use crate::scheduler;
use crate::task::ZoTask;

// Fallback TLS for non-task callers (main thread
// before `spawn`, or `std::thread::spawn`ed helpers).
// Keyed the same way as task-owned TLS.
thread_local! {
  static PTHREAD_TLS: RefCell<HashMap<u64, Vec<u8>>> =
    RefCell::new(HashMap::new());
}

// Per-task TLS map — attached to `ZoTask` via a
// side-table so the existing `ZoTask` struct stays
// lean for the common case (no TLS touched). Indexed
// by task pointer address; cleaned up when the task
// dies via `clear_for_task`.
thread_local! {
  static TASK_TLS: RefCell<HashMap<u64, HashMap<u64, Vec<u8>>>> =
    RefCell::new(HashMap::new());
}

/// Store `elem_sz` bytes under `key` for the current
/// caller.
///
/// # Safety
///
/// - `src` must point to at least `elem_sz` readable
///   bytes.
/// - The caller must keep the layout consistent
///   across get/set pairs for the same key —
///   writing 8 bytes under key K and reading 4
///   bytes back is UB from the compiler's
///   perspective.
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn _zo_tls_set(
  key: u64,
  src: *const u8,
  elem_sz: usize,
) {
  // SAFETY: caller contract on `src` + `elem_sz`.
  let bytes = unsafe { std::slice::from_raw_parts(src, elem_sz) }.to_vec();

  match scheduler::with(|s| s.current()) {
    Some(task) => {
      let task_key = task as u64;

      TASK_TLS.with(|t| {
        t.borrow_mut()
          .entry(task_key)
          .or_default()
          .insert(key, bytes);
      });
    }
    None => {
      PTHREAD_TLS.with(|t| {
        t.borrow_mut().insert(key, bytes);
      });
    }
  }
}

/// Copy `elem_sz` bytes from TLS under `key` into
/// `dst`. Returns `true` on hit, `false` on miss.
///
/// # Safety
///
/// `dst` must point to at least `elem_sz` writable
/// bytes; layout matches the paired `_zo_tls_set`.
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn _zo_tls_get(
  key: u64,
  dst: *mut u8,
  elem_sz: usize,
) -> bool {
  let bytes = match scheduler::with(|s| s.current()) {
    Some(task) => {
      let task_key = task as u64;

      TASK_TLS
        .with(|t| t.borrow().get(&task_key).and_then(|m| m.get(&key)).cloned())
    }
    None => PTHREAD_TLS.with(|t| t.borrow().get(&key).cloned()),
  };

  match bytes {
    Some(b) => {
      let n = b.len().min(elem_sz);

      // SAFETY: caller contract on `dst` + `elem_sz`.
      unsafe {
        std::ptr::copy_nonoverlapping(b.as_ptr(), dst, n);
      }

      true
    }
    None => false,
  }
}

/// Clear every TLS entry for a task. Called by the
/// runtime when a task transitions to `Dead` so its
/// TLS doesn't leak. Not a C ABI export — internal
/// to the runtime crate.
pub fn clear_for_task(task: *mut ZoTask) {
  let task_key = task as u64;

  TASK_TLS.with(|t| {
    t.borrow_mut().remove(&task_key);
  });
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn pthread_tls_set_then_get_roundtrips() {
    // Called from a non-task OS thread (the test
    // harness); falls back to PTHREAD_TLS.
    let key: u64 = 42;
    let src: u64 = 0xCAFE_BABE;
    let mut dst: u64 = 0;

    unsafe {
      _zo_tls_set(
        key,
        (&raw const src).cast::<u8>(),
        std::mem::size_of::<u64>(),
      );

      let hit = _zo_tls_get(
        key,
        (&raw mut dst).cast::<u8>(),
        std::mem::size_of::<u64>(),
      );

      assert!(hit);
    }

    assert_eq!(dst, src);
  }

  #[test]
  fn missing_key_returns_false() {
    let mut dst: u64 = 0;

    unsafe {
      let hit = _zo_tls_get(
        0xDEAD_BEEF,
        (&raw mut dst).cast::<u8>(),
        std::mem::size_of::<u64>(),
      );

      assert!(!hit);
    }

    assert_eq!(dst, 0);
  }
}
