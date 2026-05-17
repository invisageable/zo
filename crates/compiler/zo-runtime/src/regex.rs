//! regex — JS-style regular expression engine backed by
//! Rust's `regex` crate. Linear-time matching, no
//! catastrophic backtracking. Handles are 1-indexed into a
//! per-thread cache; `0` is the "compile failed" sentinel.
//! String-returning calls stage bytes in a per-thread
//! `Vec<u8>` scratch (with a trailing NUL so the pointer is
//! also a valid C-string) and expose the byte length via
//! the companion `last_str_len` FFI.

use std::cell::RefCell;
use std::os::raw::c_char;

use regex::{Regex, RegexBuilder};

/// Trailing NUL byte appended to the scratch by
/// `scratch_store`. Subtracted in `_zo_regex_last_str_len`
/// so callers see the payload length, not the buffer length.
const SCRATCH_NUL: usize = 1;

thread_local! {
  /// Compiled-regex cache. Handles are 1-indexed (`0`
  /// reserved for "compile failed / invalid handle"); the
  /// underlying `Vec` index is `handle - 1`.
  static REGEX_CACHE: RefCell<Vec<Regex>> = const { RefCell::new(Vec::new()) };

  /// Scratch buffer for the most recent string-returning
  /// call. Reused across calls — `clear` + `extend_from_slice`
  /// keeps the `Vec`'s capacity so tight `replace_all` loops
  /// don't reallocate per iteration.
  static SCRATCH: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

/// Run `f` against the regex at `handle` and return its
/// result, or `default` for `handle == 0` / out-of-range.
/// Closure form avoids the `Arc` clone every per-handle FFI
/// would otherwise pay.
fn with_regex<R>(
  handle: i64,
  default: R,
  f: impl FnOnce(&Regex) -> R,
) -> R {
  REGEX_CACHE.with(|cell| {
    let cache = cell.borrow();
    let Some(idx) = (handle as usize).checked_sub(1) else {
      return default;
    };

    cache.get(idx).map(f).unwrap_or(default)
  })
}

/// Convert a NUL-terminated `*const c_char` (zo passes a
/// `c_str` pointer for every string FFI arg) into a
/// borrowed `&str`. Returns `""` for null / non-UTF-8.
///
/// # Safety
///
/// The returned reference borrows from `ptr`; callers must
/// not let it escape past the next FFI write to the same
/// memory. All call sites in this module consume the
/// `&str` within the same function body, so the unbounded
/// lifetime is sound in practice.
unsafe fn cstr_to_str<'a>(ptr: *const c_char) -> &'a str {
  if ptr.is_null() {
    return "";
  }

  unsafe { std::ffi::CStr::from_ptr(ptr) }
    .to_str()
    .unwrap_or("")
}

/// Replace the scratch contents with `bytes` + trailing NUL
/// and return a stable pointer to the new contents. The
/// pointer is valid until the next call that touches the
/// scratch on this thread.
fn scratch_store(bytes: &[u8]) -> *const c_char {
  SCRATCH.with(|cell| {
    let mut out = cell.borrow_mut();

    out.clear();
    out.extend_from_slice(bytes);
    out.push(0);
    out.as_ptr() as *const c_char
  })
}

/// Compile a pattern with the given JS-style flag string
/// (`"i"` case-insensitive, `"m"` multiline, `"s"` dotall,
/// `"x"` extended). Returns a 1-indexed handle, `0` on
/// compile failure.
///
/// # Safety
///
/// `pattern` and `flags` must point at NUL-terminated UTF-8
/// byte sequences.
#[unsafe(export_name = "zo_regex_compile")]
pub unsafe extern "C" fn _zo_regex_compile(
  pattern: *const c_char,
  flags: *const c_char,
) -> i64 {
  let pattern = unsafe { cstr_to_str(pattern) };
  let flags = unsafe { cstr_to_str(flags) };

  let mut builder = RegexBuilder::new(pattern);

  // Flags are ASCII; byte iteration skips the UTF-8 decoder.
  for &flag in flags.as_bytes() {
    match flag {
      b'i' => {
        builder.case_insensitive(true);
      }
      b'm' => {
        builder.multi_line(true);
      }
      b's' => {
        builder.dot_matches_new_line(true);
      }
      b'x' => {
        builder.ignore_whitespace(true);
      }
      _ => {}
    }
  }

  let Ok(re) = builder.build() else { return 0 };

  REGEX_CACHE.with(|c| {
    let mut cache = c.borrow_mut();

    cache.push(re);
    cache.len() as i64
  })
}

/// `1` if `haystack` contains a match, `0` otherwise.
/// Invalid handles return `0`.
///
/// # Safety
///
/// `haystack` must be a NUL-terminated UTF-8 string.
#[unsafe(export_name = "zo_regex_matches")]
pub unsafe extern "C" fn _zo_regex_matches(
  handle: i64,
  haystack: *const c_char,
) -> i64 {
  let haystack = unsafe { cstr_to_str(haystack) };

  with_regex(handle, 0, |re| i64::from(re.is_match(haystack)))
}

/// Byte offset of the first match's start, or `-1` if no
/// match / invalid handle. Pair with `_zo_regex_find_end`
/// on the same `(handle, haystack)` to recover the slice.
///
/// # Safety
///
/// `haystack` must be a NUL-terminated UTF-8 string.
#[unsafe(export_name = "zo_regex_find")]
pub unsafe extern "C" fn _zo_regex_find(
  handle: i64,
  haystack: *const c_char,
) -> i64 {
  let haystack = unsafe { cstr_to_str(haystack) };

  with_regex(handle, -1, |re| {
    re.find(haystack).map_or(-1, |m| m.start() as i64)
  })
}

/// Byte offset of the first match's end (exclusive), or
/// `-1` if no match / invalid handle.
///
/// # Safety
///
/// `haystack` must be a NUL-terminated UTF-8 string.
#[unsafe(export_name = "zo_regex_find_end")]
pub unsafe extern "C" fn _zo_regex_find_end(
  handle: i64,
  haystack: *const c_char,
) -> i64 {
  let haystack = unsafe { cstr_to_str(haystack) };

  with_regex(handle, -1, |re| {
    re.find(haystack).map_or(-1, |m| m.end() as i64)
  })
}

/// Replace every non-overlapping match in `haystack` with
/// `replacement`. The replacement string supports `$0` /
/// `$1` / ... capture references. Returns a C-string
/// pointer into thread-local scratch; pair with
/// `_zo_regex_last_str_len` to materialise a zo str.
///
/// On match, `replace_all` returns `Cow::Owned`; the no-
/// match path returns `Cow::Borrowed(haystack)` and avoids
/// the allocation that `into_owned()` would otherwise force.
/// `Cow<str>` derefs to `&str` so `.as_bytes()` works for
/// both arms.
///
/// # Safety
///
/// `haystack` and `replacement` must be NUL-terminated
/// UTF-8 strings.
#[unsafe(export_name = "zo_regex_replace_all")]
pub unsafe extern "C" fn _zo_regex_replace_all(
  handle: i64,
  haystack: *const c_char,
  replacement: *const c_char,
) -> *const c_char {
  let haystack = unsafe { cstr_to_str(haystack) };
  let replacement = unsafe { cstr_to_str(replacement) };

  // The invalid-handle fallback resets the scratch to empty
  // so a caller reading `last_str_len` after a failure sees
  // `0`, not a stale length from the previous call.
  with_regex(handle, scratch_store(b""), |re| {
    scratch_store(re.replace_all(haystack, replacement).as_bytes())
  })
}

/// Byte length of the most recent scratch payload (excludes
/// the trailing NUL). Read immediately after a string-
/// returning call before any other regex call on this
/// thread.
#[unsafe(export_name = "zo_regex_last_str_len")]
pub extern "C" fn _zo_regex_last_str_len() -> i64 {
  SCRATCH.with(|cell| {
    let len = cell.borrow().len();

    if len == 0 { 0 } else { (len - SCRATCH_NUL) as i64 }
  })
}
