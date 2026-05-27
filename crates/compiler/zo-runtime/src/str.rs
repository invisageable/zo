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

  u64::from_le_bytes(len_bytes.try_into().unwrap()) as usize
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
  let mut buf = Box::<[u8]>::new_uninit_slice(total);

  // SAFETY: every byte of `buf` is initialized below —
  // 8 bytes of length header, `len` bytes via `fill`,
  // 1 trailing null — covering all `total` slots. The
  // `assume_init` therefore upholds the invariant.
  unsafe {
    let ptr = buf.as_mut_ptr() as *mut u8;
    let len_le = (len as u64).to_le_bytes();

    std::ptr::copy_nonoverlapping(len_le.as_ptr(), ptr, 8);

    let payload = std::slice::from_raw_parts_mut(ptr.add(8), len);

    fill(payload);

    *ptr.add(8 + len) = 0;
  }

  let init = unsafe { buf.assume_init() };

  Box::leak(init).as_ptr()
}

/// Allocate a heap-backed zo `str` containing `payload`.
/// Thin wrapper over `alloc_str_with` for callers that
/// already have a contiguous slice.
pub(crate) fn alloc_str(payload: &[u8]) -> *const u8 {
  alloc_str_with(payload.len(), |dst| dst.copy_from_slice(payload))
}

/// Allocate a fresh heap-backed zo `str` from a raw byte
/// buffer. Codegen calls this from IO finalize paths to
/// move read syscall data off a shared per-function
/// scratch buffer onto the heap, so a single scratch
/// buffer can serve every IO call in a function.
///
/// # Safety
///
/// `buf` must point at `len` readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn zo_str_alloc(
  buf: *const u8,
  len: usize,
) -> *const u8 {
  let bytes = unsafe { std::slice::from_raw_parts(buf, len) };

  alloc_str(bytes)
}

/// Decimal-format `n` as a fresh heap zo `str`. Backs
/// `core::int::to_str` so source-level code can compose
/// numbers into strings (`"Content-Length: " ++
/// body.len.to_str()`) — needed by the upcoming
/// `core::http` standard-library layer.
///
/// # Safety
///
/// No preconditions — `n` is a plain scalar.
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn zo_int_to_str(n: i64) -> *const u8 {
  let formatted = format!("{n}");

  alloc_str(formatted.as_bytes())
}

/// Convert an `f64` to its string representation.
///
/// # Safety
///
/// No preconditions — `f` is a plain scalar.
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn zo_float_to_str(f: f64) -> *const u8 {
  let formatted = format!("{f}");

  alloc_str(formatted.as_bytes())
}

/// Convert a boolean (0 = false, nonzero = true) to `str`.
///
/// @note — returns pointers to leaked static buffers,
/// no per-call allocation.
///
/// # Safety
///
/// No preconditions — `b` is a plain scalar.
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn zo_bool_to_str(b: i64) -> *const u8 {
  static TRUE_STR: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
  static FALSE_STR: std::sync::OnceLock<usize> = std::sync::OnceLock::new();

  let ptr = if b != 0 {
    *TRUE_STR.get_or_init(|| alloc_str(b"true") as usize)
  } else {
    *FALSE_STR.get_or_init(|| alloc_str(b"false") as usize)
  };

  ptr as *const u8
}

/// Convert a unicode codepoint to a single-char `str`.
///
/// # Safety
///
/// `c` must be a valid unicode scalar value.
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn zo_char_to_str(c: u32) -> *const u8 {
  let ch = char::from_u32(c).unwrap_or('\u{FFFD}');
  let mut buf = [0u8; 4];
  let encoded = ch.encode_utf8(&mut buf);

  alloc_str(encoded.as_bytes())
}

/// Concatenate N zo strs into a single heap-allocated str.
///
/// @note — single allocation regardless of segment count.
/// Used by `Insn::StringFormat` for interpolated strings.
///
/// # Safety
///
/// `ptrs` must point at an array of `count` valid zo str
/// header pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn zo_str_multi_concat(
  count: u64,
  ptrs: *const *const u8,
) -> *const u8 {
  let count = count as usize;
  let slices: &[*const u8] = unsafe { std::slice::from_raw_parts(ptrs, count) };

  let mut total: usize = 0;

  for &ptr in slices {
    total += unsafe { str_len(ptr) };
  }

  alloc_str_with(total, |dst| {
    let mut offset = 0;

    for &ptr in slices {
      let bytes = unsafe { str_bytes(ptr) };

      dst[offset..offset + bytes.len()].copy_from_slice(bytes);
      offset += bytes.len();
    }
  })
}

/// Concatenate two zo strs into a fresh heap zo str.
///
/// Replaces an earlier inline AArch64 emitter that
/// permanently lowered SP by `len(a) + len(b) + 24` and
/// never restored it — the function epilogue's fixed-
/// constant SP fix-up then left SP unbalanced, so
/// `ldp x29, x30, [sp]` read garbage and `ret` jumped
/// to a junk address (visible as a hang). Owning the
/// allocation here keeps SP stable and matches the
/// `zo_str_slice` / `alloc_str_with` lifetime model.
///
/// # Safety
///
/// `lhs` and `rhs` must both point at live zo str
/// headers (`[len:u64][bytes][NUL]`).
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn zo_str_concat(
  lhs: *const u8,
  rhs: *const u8,
) -> *const u8 {
  let lhs_bytes = unsafe { str_bytes(lhs) };
  let rhs_bytes = unsafe { str_bytes(rhs) };
  let total = lhs_bytes.len() + rhs_bytes.len();

  alloc_str_with(total, |dst| {
    dst[..lhs_bytes.len()].copy_from_slice(lhs_bytes);
    dst[lhs_bytes.len()..].copy_from_slice(rhs_bytes);
  })
}

/// Validate `s` for interior NULs and return a C-string ptr.
///
/// @note — scans the payload bytes between the 8-byte length
/// prefix and the trailing NUL. Returns the post-prefix
/// pointer when clean, null when an interior NUL would
/// silently truncate the C-side read. Backs zo's
/// `CStr::new(s)`.
///
/// # Safety
///
/// `s` must point at a valid zo str header
/// (`[len:u64][bytes][NUL]`) or be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zo_cstr_from_str(
  s: *const u8,
) -> *const std::os::raw::c_char {
  if s.is_null() {
    return std::ptr::null();
  }

  let len = unsafe { str_len(s) };
  let payload = unsafe { s.add(8) };
  let bytes = unsafe { std::slice::from_raw_parts(payload, len) };

  if bytes.contains(&0) {
    return std::ptr::null();
  }

  payload as *const std::os::raw::c_char
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
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn zo_str_slice(
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
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn zo_str_replace(
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

/// First pass of `zo_str_replace`: count non-overlapping
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
    let sliced = unsafe { zo_str_slice(src.as_ptr(), 7, 12) };

    let bytes = unsafe { str_bytes(sliced) };

    assert_eq!(bytes, b"world");
    assert_eq!(unsafe { str_len(sliced) }, 5);
  }

  #[test]
  fn slice_empty_range_produces_empty_str() {
    let src = make_str(b"abc");
    let sliced = unsafe { zo_str_slice(src.as_ptr(), 1, 1) };

    assert_eq!(unsafe { str_len(sliced) }, 0);
    assert_eq!(unsafe { str_bytes(sliced) }, b"");
  }

  #[test]
  #[should_panic(expected = "out of range")]
  fn slice_out_of_range_panics() {
    let src = make_str(b"short");

    unsafe {
      zo_str_slice(src.as_ptr(), 0, 100);
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
      unsafe { zo_str_replace(src.as_ptr(), needle.as_ptr(), with.as_ptr()) };

    assert_eq!(unsafe { str_bytes(out) }, b"a--b--c");
  }

  #[test]
  fn replace_shrinks_when_with_shorter() {
    let src = make_str(b"foo, bar, baz");
    let needle = make_str(b", ");
    let with = make_str(b",");
    let out =
      unsafe { zo_str_replace(src.as_ptr(), needle.as_ptr(), with.as_ptr()) };

    assert_eq!(unsafe { str_bytes(out) }, b"foo,bar,baz");
  }

  #[test]
  fn replace_empty_with_deletes() {
    let src = make_str(b"hello world");
    let needle = make_str(b"l");
    let with = make_str(b"");
    let out =
      unsafe { zo_str_replace(src.as_ptr(), needle.as_ptr(), with.as_ptr()) };

    assert_eq!(unsafe { str_bytes(out) }, b"heo word");
  }

  #[test]
  fn replace_empty_needle_returns_src() {
    let src = make_str(b"hello");
    let needle = make_str(b"");
    let with = make_str(b"x");
    let out =
      unsafe { zo_str_replace(src.as_ptr(), needle.as_ptr(), with.as_ptr()) };

    assert_eq!(out, src.as_ptr());
  }

  #[test]
  fn replace_no_match_returns_src() {
    let src = make_str(b"hello");
    let needle = make_str(b"xyz");
    let with = make_str(b"!");
    let out =
      unsafe { zo_str_replace(src.as_ptr(), needle.as_ptr(), with.as_ptr()) };

    assert_eq!(out, src.as_ptr());
  }

  // -------------------------------------------------
  // Property tests: interpolation runtime helpers.
  // -------------------------------------------------

  use proptest::prelude::*;
  use proptest::test_runner::{Config, FileFailurePersistence};

  proptest! {
    #![proptest_config(Config {
      failure_persistence: Some(Box::new(
        FileFailurePersistence::Off,
      )),
      cases: 500,
      ..Config::default()
    })]

    #[test]
    fn multi_concat_equals_sequential_append(
      parts in proptest::collection::vec(
        "[a-zA-Z0-9 ]{0,30}",
        1..8,
      )
    ) {
      let expected: String = parts.concat();
      let blobs: Vec<Box<[u8]>> =
        parts.iter().map(|s| make_str(s.as_bytes())).collect();
      let ptrs: Vec<*const u8> =
        blobs.iter().map(|b| b.as_ptr()).collect();
      let result = unsafe {
        zo_str_multi_concat(
          ptrs.len() as u64,
          ptrs.as_ptr(),
        )
      };
      let result_bytes = unsafe { str_bytes(result) };

      prop_assert_eq!(
        result_bytes,
        expected.as_bytes(),
        "multi_concat mismatch"
      );
    }

    #[test]
    fn int_to_str_matches_format(n in any::<i64>()) {
      let result = unsafe { zo_int_to_str(n) };
      let bytes = unsafe { str_bytes(result) };
      let actual = std::str::from_utf8(bytes).unwrap();
      let expected = format!("{n}");

      prop_assert_eq!(actual, expected.as_str());
    }

    #[test]
    fn float_to_str_matches_format(f in any::<f64>()) {
      prop_assume!(!f.is_nan());

      let result = unsafe { zo_float_to_str(f) };
      let bytes = unsafe { str_bytes(result) };
      let actual = std::str::from_utf8(bytes).unwrap();
      let expected = format!("{f}");

      prop_assert_eq!(actual, expected.as_str());
    }

    #[test]
    fn bool_to_str_correct(b in any::<bool>()) {
      let result = unsafe {
        zo_bool_to_str(if b { 1 } else { 0 })
      };
      let bytes = unsafe { str_bytes(result) };
      let actual = std::str::from_utf8(bytes).unwrap();

      prop_assert_eq!(
        actual,
        if b { "true" } else { "false" }
      );
    }

    #[test]
    fn char_to_str_correct(c in any::<char>()) {
      let result = unsafe { zo_char_to_str(c as u32) };
      let bytes = unsafe { str_bytes(result) };
      let actual = std::str::from_utf8(bytes).unwrap();
      let expected = c.to_string();

      prop_assert_eq!(actual, expected.as_str());
    }
  }

  #[test]
  fn multi_concat_single_element() {
    let src = make_str(b"hello");
    let ptrs = [src.as_ptr()];
    let result = unsafe { zo_str_multi_concat(1, ptrs.as_ptr()) };
    let bytes = unsafe { str_bytes(result) };

    assert_eq!(bytes, b"hello");
  }

  #[test]
  fn multi_concat_empty_strings() {
    let a = make_str(b"");
    let b = make_str(b"");
    let ptrs = [a.as_ptr(), b.as_ptr()];
    let result = unsafe { zo_str_multi_concat(2, ptrs.as_ptr()) };

    assert_eq!(unsafe { str_len(result) }, 0);
  }

  #[test]
  fn multi_concat_mixed_empty_and_content() {
    let a = make_str(b"hello");
    let b = make_str(b"");
    let c = make_str(b"world");
    let ptrs = [a.as_ptr(), b.as_ptr(), c.as_ptr()];
    let result = unsafe { zo_str_multi_concat(3, ptrs.as_ptr()) };
    let bytes = unsafe { str_bytes(result) };

    assert_eq!(bytes, b"helloworld");
  }
}
