//! Phase 6 of `PLAN_CHANNELS.md` — channel runtime.
//!
//! A channel is a mutex-protected FIFO queue with two
//! condition variables (senders blocked on "buffer full",
//! receivers blocked on "buffer empty"). Zero capacity
//! means unbuffered rendezvous: a sender unblocks only
//! when a matching receiver is actually waiting.
//!
//! The three `#[no_mangle] extern "C"` exports below are
//! the symbols the ARM codegen (Phase 5) emits `BL`
//! placeholders for. Once the Mach-O writer (Phase 7)
//! links `libzo_runtime.a`, programs will resolve to
//! these functions at load time.
//!
//! ABI — values are passed through raw byte pointers. The
//! compiler knows each channel's element size and hands it
//! to `_zo_chan_new` at construction time; subsequent
//! `_zo_chan_send` / `_zo_chan_recv` calls `memcpy`
//! `elem_sz` bytes through the caller-owned buffer. This
//! sidesteps the need for a runtime type descriptor — the
//! compiler is the source of truth for layout.

use std::collections::VecDeque;
use std::sync::{Condvar, Mutex};

/// Per-channel runtime state. Heap-allocated and owned
/// by a raw pointer so the ABI is a single `*mut ZoChan`.
///
/// The layout is **not** `#[repr(C)]` — zo code only ever
/// holds the opaque pointer; all field access happens
/// inside this module.
pub struct ZoChan {
  inner: Mutex<ChannelInner>,
  senders: Condvar,
  receivers: Condvar,
  elem_sz: usize,
  capacity: usize,
}

/// Data protected by the channel's mutex.
struct ChannelInner {
  /// FIFO of raw byte buffers, one per in-flight value.
  queue: VecDeque<Vec<u8>>,
  /// True when a caller has closed the channel. Send on a
  /// closed channel panics; recv drains any buffered
  /// values and then returns the zeroed buffer (Phase 6
  /// keeps it simple — a richer Option<T> return lands
  /// with the nursery-cancel wiring).
  closed: bool,
}

impl ZoChan {
  fn new(elem_sz: usize, capacity: usize) -> Self {
    Self {
      inner: Mutex::new(ChannelInner {
        queue: VecDeque::with_capacity(capacity.max(1)),
        closed: false,
      }),
      senders: Condvar::new(),
      receivers: Condvar::new(),
      elem_sz,
      capacity,
    }
  }
}

/// Allocate a fresh channel.
///
/// # Safety
///
/// The returned pointer must be released via
/// [`_zo_chan_free`]. Crossing threads is safe —
/// `ZoChan`'s fields are all `Send + Sync`.
#[unsafe(no_mangle)]
pub extern "C" fn _zo_chan_new(elem_sz: usize, capacity: usize) -> *mut ZoChan {
  Box::into_raw(Box::new(ZoChan::new(elem_sz, capacity)))
}

/// Push a value. Blocks when the bounded buffer is full.
///
/// # Safety
///
/// - `chan` must come from [`_zo_chan_new`] and still be
///   live (not yet freed).
/// - `src` must point to at least `elem_sz` bytes of
///   readable memory laid out exactly as the compiler
///   declared the channel's element type.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _zo_chan_send(chan: *mut ZoChan, src: *const u8) {
  // SAFETY: caller contract.
  let ch = unsafe { &*chan };
  let mut guard = ch.inner.lock().expect("zo-chan poisoned");

  // Wait until there's room (unbounded when capacity == 0
  // is handled as a rendezvous: one slot, receiver drains
  // immediately after the send).
  let bound = ch.capacity.max(1);

  while guard.queue.len() >= bound {
    if guard.closed {
      panic!("send on closed zo-chan");
    }

    guard = ch
      .senders
      .wait(guard)
      .expect("zo-chan senders wait poisoned");
  }

  // SAFETY: caller contract — `src..src+elem_sz` is valid
  // to read and the compiler has made `elem_sz` match
  // this channel's declared element size.
  let mut buf = vec![0u8; ch.elem_sz];

  unsafe {
    std::ptr::copy_nonoverlapping(src, buf.as_mut_ptr(), ch.elem_sz);
  }

  guard.queue.push_back(buf);
  ch.receivers.notify_one();
}

/// Pop a value. Blocks when the channel is empty.
///
/// # Safety
///
/// - `chan` must come from [`_zo_chan_new`] and still be
///   live.
/// - `dst` must point to at least `elem_sz` writable
///   bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _zo_chan_recv(chan: *mut ZoChan, dst: *mut u8) {
  // SAFETY: caller contract.
  let ch = unsafe { &*chan };
  let mut guard = ch.inner.lock().expect("zo-chan poisoned");

  while guard.queue.is_empty() {
    if guard.closed {
      // Zero-fill on closed drain — richer Option return
      // is deferred until the cancel wiring lands.
      unsafe {
        std::ptr::write_bytes(dst, 0, ch.elem_sz);
      }

      return;
    }

    guard = ch
      .receivers
      .wait(guard)
      .expect("zo-chan receivers wait poisoned");
  }

  let buf = guard.queue.pop_front().expect("zo-chan queue invariant");

  // SAFETY: buf.len() == ch.elem_sz by construction in
  // `_zo_chan_send`; dst is caller-guaranteed writable.
  unsafe {
    std::ptr::copy_nonoverlapping(buf.as_ptr(), dst, ch.elem_sz);
  }

  ch.senders.notify_one();
}

/// Release a channel.
///
/// # Safety
///
/// `chan` must have come from [`_zo_chan_new`] and must
/// not be used after this call returns.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _zo_chan_free(chan: *mut ZoChan) {
  if chan.is_null() {
    return;
  }

  // SAFETY: caller contract — exclusive ownership.
  drop(unsafe { Box::from_raw(chan) });
}

#[cfg(test)]
mod tests {
  use super::*;

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

      // `ch` is a raw pointer, not Send; wrap it in usize
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
}
