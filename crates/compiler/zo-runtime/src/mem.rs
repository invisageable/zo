//! Raw memory operations — `core/mem.zo` FFI backing.
//!
//! Thin wrappers around libc `memcpy`, `memset`, `memcmp`,
//! `malloc`, `free`. No safety checks — the zo API makes
//! the unsafety explicit via `s64` pointer arguments.

/// `mem::copy(dst, src, len)` — non-overlapping byte copy.
///
/// # Safety
///
/// `dst` and `src` must point at `len` readable/writable
/// bytes with no overlap.
#[unsafe(export_name = "zo_mem_copy")]
pub unsafe extern "C-unwind" fn _zo_mem_copy(
  dst: *mut u8,
  src: *const u8,
  len: usize,
) {
  if dst.is_null() || src.is_null() || len == 0 {
    return;
  }

  unsafe { std::ptr::copy_nonoverlapping(src, dst, len) };
}

/// `mem::set(dst, value, len)` — fill `len` bytes with
/// `value`.
///
/// # Safety
///
/// `dst` must point at `len` writable bytes.
#[unsafe(export_name = "zo_mem_set")]
pub unsafe extern "C-unwind" fn _zo_mem_set(
  dst: *mut u8,
  value: u8,
  len: usize,
) {
  if dst.is_null() || len == 0 {
    return;
  }

  unsafe { std::ptr::write_bytes(dst, value, len) };
}

/// `mem::compare(a, b, len)` — lexicographic byte compare.
/// Returns `< 0`, `0`, or `> 0`.
///
/// # Safety
///
/// Both pointers must point at `len` readable bytes.
#[unsafe(export_name = "zo_mem_compare")]
pub unsafe extern "C-unwind" fn _zo_mem_compare(
  a: *const u8,
  b: *const u8,
  len: usize,
) -> i32 {
  if a.is_null() || b.is_null() || len == 0 {
    return 0;
  }

  unsafe { libc::memcmp(a.cast(), b.cast(), len) }
}

/// `mem::alloc(size)` — heap allocate `size` bytes.
/// Returns null on failure.
///
/// # Safety
///
/// Caller must eventually `free` the returned pointer.
#[unsafe(export_name = "zo_mem_alloc")]
pub unsafe extern "C-unwind" fn _zo_mem_alloc(size: usize) -> *mut u8 {
  if size == 0 {
    return std::ptr::null_mut();
  }

  unsafe { libc::malloc(size).cast() }
}

/// `mem::free(ptr)` — release a pointer from `alloc`.
///
/// # Safety
///
/// `ptr` must have been returned by `zo_mem_alloc` (or
/// be null, which is a no-op).
#[unsafe(export_name = "zo_mem_free")]
pub unsafe extern "C-unwind" fn _zo_mem_free(ptr: *mut u8) {
  if ptr.is_null() {
    return;
  }

  unsafe { libc::free(ptr.cast()) };
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn alloc_set_compare_free() {
    unsafe {
      let a = _zo_mem_alloc(16);
      let b = _zo_mem_alloc(16);

      assert!(!a.is_null());
      assert!(!b.is_null());

      _zo_mem_set(a, 0xAA, 16);
      _zo_mem_set(b, 0xAA, 16);

      assert_eq!(_zo_mem_compare(a, b, 16), 0);

      *b = 0x00;

      assert!(_zo_mem_compare(a, b, 16) > 0);

      _zo_mem_free(a);
      _zo_mem_free(b);
    }
  }

  #[test]
  fn copy_between_buffers() {
    unsafe {
      let src = _zo_mem_alloc(8);
      let dst = _zo_mem_alloc(8);

      assert!(!src.is_null());
      assert!(!dst.is_null());

      for i in 0..8u8 {
        *src.add(i as usize) = i + 1;
      }

      _zo_mem_copy(dst, src, 8);

      assert_eq!(_zo_mem_compare(src, dst, 8), 0);

      _zo_mem_free(src);
      _zo_mem_free(dst);
    }
  }

  #[test]
  fn null_safety() {
    unsafe {
      _zo_mem_copy(std::ptr::null_mut(), std::ptr::null(), 10);
      _zo_mem_set(std::ptr::null_mut(), 0, 10);
      assert_eq!(_zo_mem_compare(std::ptr::null(), std::ptr::null(), 10), 0);
      assert!(_zo_mem_alloc(0).is_null());
      _zo_mem_free(std::ptr::null_mut());
    }
  }
}
