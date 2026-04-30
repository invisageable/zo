//! Runtime growable buffer backing zo's `Vec<T>`.
//!
//! Flat byte arena. Capacity doubles when `len == cap`.
//! Initial capacity 8 elements. Element semantics
//! (`Ty::Int`, `Ty::Str`, etc.) are opaque to the
//! runtime — the buffer stores `elem_sz` raw bytes per
//! slot. The compiler emits the call-site marshaling
//! (load 8 bytes for an int, the pointer for a str, etc.)
//! and hands the runtime a `*const u8` to copy from.
//!
//! Pointer-stability: zo holds a `*mut ZoVec` (boxed +
//! leaked); ops mutate the inner `slots` Vec but the
//! `*mut ZoVec` itself never moves. `__zo_vec_free`
//! reclaims the box.

const INITIAL_CAPACITY: usize = 8;

/// Backing struct for `Vec<T>` at the runtime ABI
/// boundary. Heap-stored byte buffer; doubling growth.
pub struct ZoVec {
  bytes: Vec<u8>,
  elem_sz: usize,
  len: usize,
  cap: usize,
}

impl ZoVec {
  fn new(elem_sz: usize, cap_hint: usize) -> Self {
    let cap = cap_hint.max(INITIAL_CAPACITY);

    Self {
      bytes: vec![0u8; cap * elem_sz],
      elem_sz,
      len: 0,
      cap,
    }
  }

  fn grow(&mut self) {
    let new_cap = self.cap * 2;
    let new_bytes_len = new_cap * self.elem_sz;

    self.bytes.resize(new_bytes_len, 0);
    self.cap = new_cap;
  }
}

/// Allocate a new vec. The returned pointer is
/// `Box::leak`-ed; the matching `__zo_vec_free` reclaims
/// it.
///
/// `elem_kind` is reserved for future use (today the
/// runtime treats every element as opaque bytes); the
/// compiler still passes it for ABI symmetry with
/// `__zo_map_new`.
///
/// # Safety
///
/// `elem_sz` must be the same width every subsequent op
/// uses. Mismatched sizes corrupt the buffer.
#[unsafe(export_name = "zo_vec_new")]
pub unsafe extern "C-unwind" fn _zo_vec_new(
  _elem_kind: u8,
  elem_sz: usize,
  cap: usize,
) -> *mut ZoVec {
  let v = ZoVec::new(elem_sz, cap);

  Box::into_raw(Box::new(v))
}

/// Append `elem_sz` bytes from `val_ptr` to the end of
/// the vec. Grows the backing buffer if needed.
///
/// # Safety
///
/// `vec` must be live; `val_ptr` must point at at least
/// `elem_sz` readable bytes.
#[unsafe(export_name = "zo_vec_push")]
pub unsafe extern "C-unwind" fn _zo_vec_push(
  vec: *mut ZoVec,
  val_ptr: *const u8,
) {
  let v = unsafe { &mut *vec };

  if v.len == v.cap {
    v.grow();
  }

  let off = v.len * v.elem_sz;

  unsafe {
    std::ptr::copy_nonoverlapping(
      val_ptr,
      v.bytes.as_mut_ptr().add(off),
      v.elem_sz,
    );
  }

  v.len += 1;
}

/// Pop the last element into `val_out`. Returns `true`
/// on success, `false` when the vec is empty (in which
/// case `val_out` is left untouched).
///
/// # Safety
///
/// `vec` must be live; `val_out` must point at at least
/// `elem_sz` writable bytes.
#[unsafe(export_name = "zo_vec_pop")]
pub unsafe extern "C-unwind" fn _zo_vec_pop(
  vec: *mut ZoVec,
  val_out: *mut u8,
) -> bool {
  let v = unsafe { &mut *vec };

  if v.len == 0 {
    return false;
  }

  v.len -= 1;

  let off = v.len * v.elem_sz;

  unsafe {
    std::ptr::copy_nonoverlapping(
      v.bytes.as_ptr().add(off),
      val_out,
      v.elem_sz,
    );
  }

  true
}

/// Read the element at `idx` into `val_out`. Returns
/// `true` on hit, `false` on out-of-bounds.
///
/// # Safety
///
/// As `__zo_vec_pop`.
#[unsafe(export_name = "zo_vec_get")]
pub unsafe extern "C-unwind" fn _zo_vec_get(
  vec: *mut ZoVec,
  idx: usize,
  val_out: *mut u8,
) -> bool {
  let v = unsafe { &*vec };

  if idx >= v.len {
    return false;
  }

  let off = idx * v.elem_sz;

  unsafe {
    std::ptr::copy_nonoverlapping(
      v.bytes.as_ptr().add(off),
      val_out,
      v.elem_sz,
    );
  }

  true
}

/// Overwrite the element at `idx` with `elem_sz` bytes
/// from `val_ptr`. Returns `true` on hit, `false` on
/// out-of-bounds.
///
/// # Safety
///
/// `vec` must be live; `val_ptr` must point at at least
/// `elem_sz` readable bytes.
#[unsafe(export_name = "zo_vec_set")]
pub unsafe extern "C-unwind" fn _zo_vec_set(
  vec: *mut ZoVec,
  idx: usize,
  val_ptr: *const u8,
) -> bool {
  let v = unsafe { &mut *vec };

  if idx >= v.len {
    return false;
  }

  let off = idx * v.elem_sz;

  unsafe {
    std::ptr::copy_nonoverlapping(
      val_ptr,
      v.bytes.as_mut_ptr().add(off),
      v.elem_sz,
    );
  }

  true
}

/// Remove and return the element at `idx`. Shifts every
/// element after `idx` left by one slot to keep the vec
/// contiguous (O(n) for the tail). Returns `true` on
/// success and copies the removed bytes into `val_out`;
/// returns `false` on out-of-bounds (and leaves `val_out`
/// untouched).
///
/// # Safety
///
/// As `__zo_vec_pop`.
#[unsafe(export_name = "zo_vec_remove")]
pub unsafe extern "C-unwind" fn _zo_vec_remove(
  vec: *mut ZoVec,
  idx: usize,
  val_out: *mut u8,
) -> bool {
  let v = unsafe { &mut *vec };

  if idx >= v.len {
    return false;
  }

  // Read-out and tail shift share a single `as_mut_ptr`
  // base — both target the same buffer, so a single
  // mutable handle keeps the provenance simple.
  let off = idx * v.elem_sz;
  let elem_sz = v.elem_sz;
  let tail_start = (idx + 1) * elem_sz;
  let tail_end = v.len * elem_sz;
  let count = tail_end - tail_start;

  unsafe {
    let base = v.bytes.as_mut_ptr();

    // Copy the removed element into the caller's slot.
    std::ptr::copy_nonoverlapping(base.add(off), val_out, elem_sz);

    // Shift the tail down by one. `copy` (memmove) handles
    // the overlap when removing anything but the last slot.
    if count > 0 {
      std::ptr::copy(base.add(tail_start), base.add(off), count);
    }
  }

  v.len -= 1;

  true
}

/// Number of elements in the vec.
///
/// # Safety
///
/// `vec` must be live.
#[unsafe(export_name = "zo_vec_len")]
pub unsafe extern "C-unwind" fn _zo_vec_len(vec: *mut ZoVec) -> usize {
  let v = unsafe { &*vec };

  v.len
}

/// Free the vec. The pointer must NOT be used after this
/// call.
///
/// # Safety
///
/// `vec` must be a pointer from `__zo_vec_new` that
/// hasn't been freed.
#[unsafe(export_name = "zo_vec_free")]
pub unsafe extern "C-unwind" fn _zo_vec_free(vec: *mut ZoVec) {
  if !vec.is_null() {
    unsafe {
      let _ = Box::from_raw(vec);
    }
  }
}

/// Pretty-print every live element of `vec` as
/// `[e0, e1, ...]` to `fd`. Same shape as
/// `_zo_map_show`: walk the live entries, format each
/// with `MapFmt::format_bytes` against the per-element
/// kind (set by codegen at `Vec::new` resolution time),
/// and emit a single buffered `libc::write` so partial
/// syscalls can't tear an entry across reads.
///
/// `elem_fmt` is the `MapFmt` discriminant; it tells the
/// formatter how to interpret the `elem_sz`-byte slot
/// payload.
///
/// # Safety
///
/// `vec` must be a live pointer from `__zo_vec_new`.
#[unsafe(export_name = "zo_vec_show")]
pub unsafe extern "C-unwind" fn _zo_vec_show(
  vec: *mut ZoVec,
  fd: usize,
  elem_fmt: u8,
) {
  let v = unsafe { &*vec };
  let fmt = crate::map::MapFmt::from_u8(elem_fmt);

  let mut out: Vec<u8> = Vec::with_capacity(64);

  out.push(b'[');

  for i in 0..v.len {
    if i > 0 {
      out.extend_from_slice(b", ");
    }

    let off = i * v.elem_sz;
    let slot = &v.bytes[off..off + v.elem_sz];

    // `is_value = true` so str payloads dereference the
    // slot's stored header pointer rather than reading
    // the slot bytes as raw UTF-8 (mirrors `MapFmt::Str`'s
    // value-side treatment in `_zo_map_show`).
    fmt.format_bytes(slot, true, &mut out);
  }

  out.push(b']');

  unsafe {
    libc::write(fd as i32, out.as_ptr() as *const _, out.len());
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  /// Helper: pack an i64 as little-endian bytes into a
  /// fixed-size buffer the same shape codegen emits.
  fn make_int(v: i64) -> [u8; 8] {
    v.to_le_bytes()
  }

  #[test]
  fn empty_vec_len_zero() {
    let v = unsafe { _zo_vec_new(0, 8, 0) };

    assert_eq!(unsafe { _zo_vec_len(v) }, 0);

    unsafe {
      _zo_vec_free(v);
    }
  }

  #[test]
  fn push_pop_round_trip() {
    let v = unsafe { _zo_vec_new(0, 8, 0) };

    for i in 0..5i64 {
      let buf = make_int(i + 100);

      unsafe {
        _zo_vec_push(v, buf.as_ptr());
      }
    }

    assert_eq!(unsafe { _zo_vec_len(v) }, 5);

    let mut out = [0u8; 8];

    for expected in (0..5i64).rev() {
      let ok = unsafe { _zo_vec_pop(v, out.as_mut_ptr()) };

      assert!(ok);
      assert_eq!(i64::from_le_bytes(out), expected + 100);
    }

    assert_eq!(unsafe { _zo_vec_len(v) }, 0);

    let ok = unsafe { _zo_vec_pop(v, out.as_mut_ptr()) };

    assert!(!ok);

    unsafe {
      _zo_vec_free(v);
    }
  }

  #[test]
  fn grows_past_initial_capacity() {
    let v = unsafe { _zo_vec_new(0, 8, 0) };

    for i in 0..32i64 {
      let buf = make_int(i);

      unsafe {
        _zo_vec_push(v, buf.as_ptr());
      }
    }

    assert_eq!(unsafe { _zo_vec_len(v) }, 32);

    let mut out = [0u8; 8];

    for i in 0..32i64 {
      let ok = unsafe { _zo_vec_get(v, i as usize, out.as_mut_ptr()) };

      assert!(ok);
      assert_eq!(i64::from_le_bytes(out), i);
    }

    unsafe {
      _zo_vec_free(v);
    }
  }

  #[test]
  fn get_set_round_trip() {
    let v = unsafe { _zo_vec_new(0, 8, 0) };

    for i in 0..3i64 {
      let buf = make_int(i);

      unsafe {
        _zo_vec_push(v, buf.as_ptr());
      }
    }

    let new_buf = make_int(999);
    let ok = unsafe { _zo_vec_set(v, 1, new_buf.as_ptr()) };

    assert!(ok);

    let mut out = [0u8; 8];
    let ok = unsafe { _zo_vec_get(v, 1, out.as_mut_ptr()) };

    assert!(ok);
    assert_eq!(i64::from_le_bytes(out), 999);

    let oob = unsafe { _zo_vec_set(v, 99, new_buf.as_ptr()) };

    assert!(!oob);

    let oob = unsafe { _zo_vec_get(v, 99, out.as_mut_ptr()) };

    assert!(!oob);

    unsafe {
      _zo_vec_free(v);
    }
  }
}
