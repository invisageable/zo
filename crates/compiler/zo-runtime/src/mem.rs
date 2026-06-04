//! Unchecked libc memory FFI backing `core/mem.zo`.

/// `mem::copy(dst, src, len)` — non-overlapping byte copy.
///
/// # Safety
///
/// `dst` and `src` must point at `len` readable/writable
/// bytes with no overlap.
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn zo_mem_copy(
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
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn zo_mem_set(
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
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn zo_mem_compare(
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
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn zo_mem_alloc(size: usize) -> *mut u8 {
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
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn zo_mem_free(ptr: *mut u8) {
  if ptr.is_null() {
    return;
  }

  unsafe { libc::free(ptr.cast()) };
}

/// Write one byte at `dst`.
///
/// # Safety
///
/// `dst` must point at one writable byte.
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn zo_mem_write_u8(dst: *mut u8, value: u8) {
  if !dst.is_null() {
    unsafe { *dst = value };
  }
}

/// Read one byte at `src`.
///
/// # Safety
///
/// `src` must point at one readable byte.
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn zo_mem_read_u8(src: *const u8) -> u8 {
  if src.is_null() { 0 } else { unsafe { *src } }
}

/// Write an `f64` (8 bytes, native endian) at `dst`.
/// Unaligned-safe so callers can pack floats at arbitrary
/// byte offsets in a raw buffer.
///
/// # Safety
///
/// `dst` must point at 8 writable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn zo_mem_write_f64(dst: *mut u8, value: f64) {
  if !dst.is_null() {
    unsafe { dst.cast::<f64>().write_unaligned(value) };
  }
}

/// Read an `f64` (8 bytes, native endian) at `src`.
///
/// # Safety
///
/// `src` must point at 8 readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn zo_mem_read_f64(src: *const u8) -> f64 {
  if src.is_null() {
    0.0
  } else {
    unsafe { src.cast::<f64>().read_unaligned() }
  }
}

/// Resize a heap allocation. Returns the new pointer.
///
/// # Safety
///
/// `ptr` must have been returned by `zo_mem_alloc`
/// (or be null, which acts as `alloc`).
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn zo_mem_realloc(
  ptr: *mut u8,
  new_size: usize,
) -> *mut u8 {
  if new_size == 0 {
    return std::ptr::null_mut();
  }

  unsafe { libc::realloc(ptr.cast(), new_size).cast() }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn alloc_set_compare_free() {
    unsafe {
      let a = zo_mem_alloc(16);
      let b = zo_mem_alloc(16);

      assert!(!a.is_null());
      assert!(!b.is_null());

      zo_mem_set(a, 0xAA, 16);
      zo_mem_set(b, 0xAA, 16);

      assert_eq!(zo_mem_compare(a, b, 16), 0);

      *b = 0x00;

      assert!(zo_mem_compare(a, b, 16) > 0);

      zo_mem_free(a);
      zo_mem_free(b);
    }
  }

  #[test]
  fn copy_between_buffers() {
    unsafe {
      let src = zo_mem_alloc(8);
      let dst = zo_mem_alloc(8);

      assert!(!src.is_null());
      assert!(!dst.is_null());

      for i in 0..8u8 {
        *src.add(i as usize) = i + 1;
      }

      zo_mem_copy(dst, src, 8);

      assert_eq!(zo_mem_compare(src, dst, 8), 0);

      zo_mem_free(src);
      zo_mem_free(dst);
    }
  }

  #[test]
  fn null_safety() {
    unsafe {
      zo_mem_copy(std::ptr::null_mut(), std::ptr::null(), 10);
      zo_mem_set(std::ptr::null_mut(), 0, 10);
      assert_eq!(zo_mem_compare(std::ptr::null(), std::ptr::null(), 10), 0);
      assert!(zo_mem_alloc(0).is_null());
      zo_mem_free(std::ptr::null_mut());
    }
  }

  #[test]
  fn write_read_u8() {
    unsafe {
      let buf = zo_mem_alloc(4);

      zo_mem_write_u8(buf, 0xAB);
      zo_mem_write_u8(buf.add(1), 0xCD);

      assert_eq!(zo_mem_read_u8(buf), 0xAB);
      assert_eq!(zo_mem_read_u8(buf.add(1)), 0xCD);

      zo_mem_free(buf);
    }
  }

  #[test]
  fn read_u8_null_returns_zero() {
    unsafe {
      assert_eq!(zo_mem_read_u8(std::ptr::null()), 0);
    }
  }

  #[test]
  fn write_read_f64_roundtrip() {
    unsafe {
      let buf = zo_mem_alloc(24);

      zo_mem_write_f64(buf, -0.743643);
      zo_mem_write_f64(buf.add(8), 0.131825);
      zo_mem_write_f64(buf.add(16), 1.0e-9);

      assert_eq!(zo_mem_read_f64(buf), -0.743643);
      assert_eq!(zo_mem_read_f64(buf.add(8)), 0.131825);
      assert_eq!(zo_mem_read_f64(buf.add(16)), 1.0e-9);
      assert_eq!(zo_mem_read_f64(std::ptr::null()), 0.0);

      zo_mem_free(buf);
    }
  }

  #[test]
  fn realloc_preserves_content() {
    unsafe {
      let buf = zo_mem_alloc(8);

      for i in 0..8u8 {
        *buf.add(i as usize) = i + 1;
      }

      let grown = zo_mem_realloc(buf, 32);

      assert!(!grown.is_null());

      for i in 0..8u8 {
        assert_eq!(*grown.add(i as usize), i + 1);
      }

      zo_mem_free(grown);
    }
  }

  #[test]
  fn realloc_zero_returns_null() {
    unsafe {
      assert!(zo_mem_realloc(std::ptr::null_mut(), 0).is_null());
    }
  }
}
