//! Runtime string operations — slicing.
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

/// Allocate a fresh heap-backed zo `str` of `len` bytes,
/// invoking `fill` to write the byte payload. Single
/// allocation: the closure writes directly into the output
/// buffer, with no intermediate `Vec<u8>` + copy.
///
/// Used by builders that know the final length up front
/// but assemble the bytes in pieces (replace, future
/// concat, future `format`).
///
/// Leaks the `Box` — a program-long lifetime, matching
/// the current absence of a free path.
pub(crate) fn alloc_str_with(
  len: usize,
  fill: impl FnOnce(&mut [u8]),
) -> *const u8 {
  let total = 8 + len + 1;

  let mut buf = vec![0u8; total].into_boxed_slice();

  buf[0..8].copy_from_slice(&(len as u64).to_le_bytes());
  fill(&mut buf[8..8 + len]);
  // trailing null already zero.

  let raw = Box::leak(buf);

  raw.as_ptr()
}

/// Allocate a heap-backed zo `str` containing `payload`.
/// Thin wrapper over `alloc_str_with` for callers that
/// already have a contiguous slice.
pub(crate) fn alloc_str(payload: &[u8]) -> *const u8 {
  alloc_str_with(payload.len(), |dst| dst.copy_from_slice(payload))
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

/// Replace every non-overlapping occurrence of `needle` in
/// `src` with `with`, returning a freshly allocated zo str.
///
/// Two-pass:
///   1. Count occurrences and compute `out_len`.
///   2. Allocate once, copy chunks + `with` per occurrence.
///
/// Avoids the O(n²) cost of repeated `str + str` concat
/// (each `+` allocates a new buffer holding the running
/// total). Empty `needle` returns `src` unchanged — matches
/// Rust / Python semantics; replacing "between every byte"
/// is rarely what callers want and is easy to express
/// explicitly when it is.
///
/// # Safety
///
/// `src`, `needle`, `with` must all be live zo str
/// headers.
#[unsafe(export_name = "zo_str_replace")]
pub unsafe extern "C-unwind" fn _zo_str_replace(
  src: *const u8,
  needle: *const u8,
  with: *const u8,
) -> *const u8 {
  let s = unsafe { str_bytes(src) };
  let n = unsafe { str_bytes(needle) };
  let w = unsafe { str_bytes(with) };

  if n.is_empty() || n.len() > s.len() {
    return src;
  }

  let count = count_occurrences(s, n);

  if count == 0 {
    return src;
  }

  let out_len = if w.len() >= n.len() {
    s.len() + count * (w.len() - n.len())
  } else {
    s.len() - count * (n.len() - w.len())
  };

  alloc_str_with(out_len, |dst| {
    let mut written = 0;
    let mut i = 0;
    let mut start = 0;

    while i + n.len() <= s.len() {
      if s[i..i + n.len()] == *n {
        let chunk = i - start;

        dst[written..written + chunk].copy_from_slice(&s[start..i]);
        written += chunk;

        dst[written..written + w.len()].copy_from_slice(w);
        written += w.len();

        i += n.len();
        start = i;
      } else {
        i += 1;
      }
    }

    let tail = s.len() - start;

    dst[written..written + tail].copy_from_slice(&s[start..]);
    debug_assert_eq!(written + tail, dst.len());
  })
}

/// First pass of `_zo_str_replace`: count non-overlapping
/// occurrences of `n` in `s`. Factored so the second pass
/// can reuse the exact same scan loop and the count is
/// testable without a runtime alloc.
fn count_occurrences(s: &[u8], n: &[u8]) -> usize {
  let mut count = 0;
  let mut i = 0;

  while i + n.len() <= s.len() {
    if s[i..i + n.len()] == *n {
      count += 1;
      i += n.len();
    } else {
      i += 1;
    }
  }

  count
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
  fn count_occurrences_disjoint() {
    assert_eq!(count_occurrences(b"abababab", b"ab"), 4);
  }

  #[test]
  fn count_occurrences_skips_overlap() {
    // `aaaa` with needle `aa` — non-overlapping is 2,
    // overlapping would be 3. We pick non-overlapping
    // because it matches Rust's `str::replace` semantics
    // and keeps the second-pass length math closed-form.
    assert_eq!(count_occurrences(b"aaaa", b"aa"), 2);
  }

  #[test]
  fn count_occurrences_no_match() {
    assert_eq!(count_occurrences(b"hello", b"xyz"), 0);
  }

  #[test]
  fn replace_grows_when_with_longer() {
    let src = make_str(b"a-b-c");
    let needle = make_str(b"-");
    let with = make_str(b"--");
    let out =
      unsafe { _zo_str_replace(src.as_ptr(), needle.as_ptr(), with.as_ptr()) };

    assert_eq!(unsafe { str_bytes(out) }, b"a--b--c");
  }

  #[test]
  fn replace_shrinks_when_with_shorter() {
    let src = make_str(b"foo, bar, baz");
    let needle = make_str(b", ");
    let with = make_str(b",");
    let out =
      unsafe { _zo_str_replace(src.as_ptr(), needle.as_ptr(), with.as_ptr()) };

    assert_eq!(unsafe { str_bytes(out) }, b"foo,bar,baz");
  }

  #[test]
  fn replace_empty_with_deletes() {
    let src = make_str(b"hello world");
    let needle = make_str(b"l");
    let with = make_str(b"");
    let out =
      unsafe { _zo_str_replace(src.as_ptr(), needle.as_ptr(), with.as_ptr()) };

    assert_eq!(unsafe { str_bytes(out) }, b"heo word");
  }

  #[test]
  fn replace_empty_needle_returns_src() {
    let src = make_str(b"hello");
    let needle = make_str(b"");
    let with = make_str(b"x");
    let out =
      unsafe { _zo_str_replace(src.as_ptr(), needle.as_ptr(), with.as_ptr()) };

    assert_eq!(out, src.as_ptr());
  }

  #[test]
  fn replace_no_match_returns_src() {
    let src = make_str(b"hello");
    let needle = make_str(b"xyz");
    let with = make_str(b"!");
    let out =
      unsafe { _zo_str_replace(src.as_ptr(), needle.as_ptr(), with.as_ptr()) };

    assert_eq!(out, src.as_ptr());
  }
}
