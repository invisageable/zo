//! Runtime string operations — slicing, equality.
//!
//! Zo's `str` at runtime is a pointer to a length-
//! prefixed blob: `[len: u64][bytes...][null]`. The
//! length header lives at offset 0; the bytes at
//! offset 8; a trailing null keeps C FFI happy. A
//! `str` value in a register is the pointer to the
//! header.
//!
//! Compile-time literal strings live in the binary's
//! data segment, emitted once per interned symbol by
//! the codegen. Runtime-constructed strings (a slice,
//! a concat) land on the heap via [`alloc_str`] —
//! same layout, allocated as `Box<[u8]>` and leaked
//! for the process lifetime. A proper free path waits
//! on reference counting or GC.

/// Reads the `u64` length prefix at offset 0 of a
/// runtime `str` pointer.
///
/// # Safety
///
/// `ptr` must point at a valid zo str header —
/// 8 bytes of little-endian length followed by at
/// least that many bytes of payload.
#[inline]
pub unsafe fn str_len(ptr: *const u8) -> usize {
  let len_bytes = unsafe { std::slice::from_raw_parts(ptr, 8) };

  u64::from_le_bytes([
    len_bytes[0],
    len_bytes[1],
    len_bytes[2],
    len_bytes[3],
    len_bytes[4],
    len_bytes[5],
    len_bytes[6],
    len_bytes[7],
  ]) as usize
}

/// Borrows the byte payload of a runtime `str`, not
/// including the header or trailing null.
///
/// # Safety
///
/// Same contract as [`str_len`].
#[inline]
pub unsafe fn str_bytes<'a>(ptr: *const u8) -> &'a [u8] {
  let len = unsafe { str_len(ptr) };

  unsafe { std::slice::from_raw_parts(ptr.add(8), len) }
}

/// Allocate a new heap-backed zo `str` with the given
/// byte payload. Leaks the `Box` — a program-long
/// lifetime, matching the current absence of a free
/// path.
fn alloc_str(payload: &[u8]) -> *const u8 {
  let len = payload.len();
  let total = 8 + len + 1;

  let mut buf = vec![0u8; total].into_boxed_slice();

  buf[0..8].copy_from_slice(&(len as u64).to_le_bytes());
  buf[8..8 + len].copy_from_slice(payload);
  // trailing null already zero.

  let raw = Box::leak(buf);

  raw.as_ptr()
}

/// Slice `src[lo..hi]` — produce a fresh heap str
/// containing the bytes of `src` from index `lo`
/// (inclusive) to `hi` (exclusive).
///
/// Aborts on out-of-range bounds or `lo > hi`. A
/// richer diagnostic surface waits on zo's panic
/// infrastructure; for now we fail loud so bugs
/// don't silently produce wrong results.
///
/// # Safety
///
/// `src` must be a live zo str header. `lo` and `hi`
/// must both be within `[0, str_len(src)]` and
/// `lo <= hi`.
#[unsafe(export_name = "zo_str_slice")]
pub unsafe extern "C-unwind" fn _zo_str_slice(
  src: *const u8,
  lo: usize,
  hi: usize,
) -> *const u8 {
  let src_bytes = unsafe { str_bytes(src) };

  if lo > hi || hi > src_bytes.len() {
    panic!(
      "zo_str_slice: out of range (lo={lo}, hi={hi}, len={})",
      src_bytes.len(),
    );
  }

  alloc_str(&src_bytes[lo..hi])
}

/// Compare two zo `str` values byte-wise. Returns
/// `true` iff they have the same length and same
/// bytes.
///
/// # Safety
///
/// Both `a` and `b` must be live zo str headers.
#[unsafe(export_name = "zo_str_eq")]
pub unsafe extern "C-unwind" fn _zo_str_eq(a: *const u8, b: *const u8) -> bool {
  if a == b {
    return true;
  }

  let ab = unsafe { str_bytes(a) };
  let bb = unsafe { str_bytes(b) };

  ab == bb
}

#[cfg(test)]
mod tests {
  use super::*;

  /// Build a fixed str blob the same shape the codegen
  /// emits: `[len: u64 LE][bytes][null]`.
  fn make_str(s: &[u8]) -> Box<[u8]> {
    let mut v = Vec::with_capacity(8 + s.len() + 1);

    v.extend_from_slice(&(s.len() as u64).to_le_bytes());
    v.extend_from_slice(s);
    v.push(0);
    v.into_boxed_slice()
  }

  #[test]
  fn slice_reads_len_and_copies_bytes() {
    let src = make_str(b"hello, world");
    let sliced = unsafe { _zo_str_slice(src.as_ptr(), 7, 12) };

    let bytes = unsafe { str_bytes(sliced) };

    assert_eq!(bytes, b"world");
    assert_eq!(unsafe { str_len(sliced) }, 5);
  }

  #[test]
  fn slice_empty_range_produces_empty_str() {
    let src = make_str(b"abc");
    let sliced = unsafe { _zo_str_slice(src.as_ptr(), 1, 1) };

    assert_eq!(unsafe { str_len(sliced) }, 0);
    assert_eq!(unsafe { str_bytes(sliced) }, b"");
  }

  #[test]
  #[should_panic(expected = "out of range")]
  fn slice_out_of_range_panics() {
    let src = make_str(b"short");

    unsafe {
      _zo_str_slice(src.as_ptr(), 0, 100);
    }
  }

  #[test]
  fn eq_same_contents_different_pointers() {
    let a = make_str(b"hello");
    let b = make_str(b"hello");

    assert!(unsafe { _zo_str_eq(a.as_ptr(), b.as_ptr()) });
  }

  #[test]
  fn eq_different_contents() {
    let a = make_str(b"hello");
    let b = make_str(b"world");

    assert!(!unsafe { _zo_str_eq(a.as_ptr(), b.as_ptr()) });
  }

  #[test]
  fn eq_different_lengths() {
    let a = make_str(b"hello");
    let b = make_str(b"hell");

    assert!(!unsafe { _zo_str_eq(a.as_ptr(), b.as_ptr()) });
  }
}
