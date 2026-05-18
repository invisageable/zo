//! JS-style regex engine backed by Rust's `regex` crate.
//!
//! Handles are 1-indexed; `0` is the "invalid / no match"
//! sentinel. String returns ride a 16B `(ptr, len)` struct
//! (`CBytes` on the zo side) lifted through the AAPCS
//! `Composite` return path so the zo wrapper materialises a
//! real `str` via `.to_str()` without a companion FFI call.

use zo_c_abi::{CBytes, cstr_to_str, stage_cbytes};

use std::borrow::Cow;
use std::cell::RefCell;
use std::os::raw::c_char;
use std::thread::LocalKey;

use regex::{Regex, RegexBuilder};

/// Thread-local cache backing `with_slot` / `free_slot`.
type SlotCache<T> = LocalKey<RefCell<Vec<Option<T>>>>;

/// Owned capture data for one regex match.
///
/// @note — `groups[0]` is the whole match, `groups[1..]`
/// are capture groups. `None` entries appear for non-
/// participating groups in alternations like `(a)|(b)`.
/// Strings are cloned out of `regex::Captures` so a `Match`
/// survives the haystack buffer being freed.
struct MatchSlot {
  start: i64,
  end: i64,
  groups: Vec<Option<String>>,
}

thread_local! {
  /// Compiled-regex cache, 1-indexed; `None` slots are freed.
  static REGEX_CACHE: RefCell<Vec<Option<Regex>>> =
    const { RefCell::new(Vec::new()) };

  /// Captured-match cache, mirrors `REGEX_CACHE`'s shape.
  static MATCH_CACHE: RefCell<Vec<Option<MatchSlot>>> =
    const { RefCell::new(Vec::new()) };

  /// Per-thread byte buffer for the most recent string return.
  static SCRATCH: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

/// Borrow the slot at `handle` and run `f`, else `default()`.
///
/// @note — `default` is lazy so cheap Copy paths stay zero-
/// cost (the closure inlines), while side-effectful fallbacks
/// like `CBytes::empty` only fire when the slot is actually
/// missing.
fn with_slot<T, R>(
  cache: &'static SlotCache<T>,
  handle: i64,
  default: impl FnOnce() -> R,
  f: impl FnOnce(&T) -> R,
) -> R {
  cache.with(|cell| {
    let cache = cell.borrow();
    let Some(idx) = (handle as usize).checked_sub(1) else {
      return default();
    };

    cache
      .get(idx)
      .and_then(|slot| slot.as_ref())
      .map(f)
      .unwrap_or_else(default)
  })
}

/// Write `None` at `handle - 1`; idempotent.
fn free_slot<T>(cache: &'static SlotCache<T>, handle: i64) {
  cache.with(|cell| {
    let Some(idx) = (handle as usize).checked_sub(1) else {
      return;
    };

    let mut cache = cell.borrow_mut();
    if let Some(slot) = cache.get_mut(idx) {
      *slot = None;
    }
  });
}

/// Run `op` over the regex at `handle` against `haystack`.
///
/// @note — `Cow::Borrowed` (no-match) passes through without
/// forcing an allocation; the FFI wrappers handle the
/// pointer-to-str conversion so this helper stays safe.
fn replace_via<'h>(
  handle: i64,
  haystack: &'h str,
  replacement: &str,
  op: impl FnOnce(&Regex, &'h str, &str) -> Cow<'h, str>,
) -> CBytes {
  with_slot(&REGEX_CACHE, handle, CBytes::empty, |re| {
    stage_cbytes(&SCRATCH, op(re, haystack, replacement).as_bytes())
  })
}

/// Compile a pattern with JS-style flags; return 1-indexed handle.
///
/// @note — flags: `i` case-insensitive, `m` multiline, `s`
/// dotall, `x` extended. `g`/`y`/`u` are accepted for paste-
/// from-JS compatibility but have no effect. Returns `0` on
/// compile failure OR when `pattern` is null (the zo-side
/// `CStr::new` sentinel for interior-NUL inputs).
///
/// # Safety
///
/// `pattern` and `flags` must be NUL-terminated UTF-8 or null.
#[unsafe(export_name = "zo_regex_compile")]
pub unsafe extern "C" fn _zo_regex_compile(
  pattern: *const c_char,
  flags: *const c_char,
) -> i64 {
  if pattern.is_null() {
    return 0;
  }

  let pattern = unsafe { cstr_to_str(pattern) };
  let flags = unsafe { cstr_to_str(flags) };

  let mut builder = RegexBuilder::new(pattern);

  // Flags are ASCII; byte iteration skips UTF-8 decode.
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

    cache.push(Some(re));
    cache.len() as i64
  })
}

/// Free the compiled regex at `handle`; idempotent.
#[unsafe(export_name = "zo_regex_free")]
pub extern "C" fn _zo_regex_free(handle: i64) {
  free_slot(&REGEX_CACHE, handle);
}

/// `1` when `haystack` contains a match, `0` otherwise.
///
/// # Safety
///
/// `haystack` must be a NUL-terminated UTF-8 string or null.
#[unsafe(export_name = "zo_regex_matches")]
pub unsafe extern "C" fn _zo_regex_matches(
  handle: i64,
  haystack: *const c_char,
) -> i64 {
  let haystack = unsafe { cstr_to_str(haystack) };

  with_slot(
    &REGEX_CACHE,
    handle,
    || 0,
    |re| i64::from(re.is_match(haystack)),
  )
}

/// Byte offset of the first match's start, or `-1`.
///
/// # Safety
///
/// `haystack` must be a NUL-terminated UTF-8 string or null.
#[unsafe(export_name = "zo_regex_find")]
pub unsafe extern "C" fn _zo_regex_find(
  handle: i64,
  haystack: *const c_char,
) -> i64 {
  let haystack = unsafe { cstr_to_str(haystack) };

  with_slot(
    &REGEX_CACHE,
    handle,
    || -1,
    |re| re.find(haystack).map_or(-1, |m| m.start() as i64),
  )
}

/// Byte offset of the first match's end (exclusive), or `-1`.
///
/// # Safety
///
/// `haystack` must be a NUL-terminated UTF-8 string or null.
#[unsafe(export_name = "zo_regex_find_end")]
pub unsafe extern "C" fn _zo_regex_find_end(
  handle: i64,
  haystack: *const c_char,
) -> i64 {
  let haystack = unsafe { cstr_to_str(haystack) };

  with_slot(
    &REGEX_CACHE,
    handle,
    || -1,
    |re| re.find(haystack).map_or(-1, |m| m.end() as i64),
  )
}

/// Find the first match and return a 1-indexed `Match` handle.
///
/// @note — captures are cloned out of `regex::Captures` so
/// the returned handle survives the haystack buffer being
/// freed. Returns `0` on no match / invalid regex.
///
/// # Safety
///
/// `haystack` must be a NUL-terminated UTF-8 string or null.
#[unsafe(export_name = "zo_regex_exec")]
pub unsafe extern "C" fn _zo_regex_exec(
  handle: i64,
  haystack: *const c_char,
) -> i64 {
  let haystack = unsafe { cstr_to_str(haystack) };

  let slot = with_slot(
    &REGEX_CACHE,
    handle,
    || None,
    |re| {
      let caps = re.captures(haystack)?;
      let whole = caps.get(0)?;
      let groups = (0..caps.len())
        .map(|i| caps.get(i).map(|m| m.as_str().to_owned()))
        .collect();

      Some(MatchSlot {
        start: whole.start() as i64,
        end: whole.end() as i64,
        groups,
      })
    },
  );

  let Some(slot) = slot else { return 0 };

  MATCH_CACHE.with(|cell| {
    let mut cache = cell.borrow_mut();

    cache.push(Some(slot));
    cache.len() as i64
  })
}

/// Free the match at `handle`; idempotent.
#[unsafe(export_name = "zo_regex_match_free")]
pub extern "C" fn _zo_regex_match_free(handle: i64) {
  free_slot(&MATCH_CACHE, handle);
}

/// Byte offset of the match's start, or `-1` if freed.
#[unsafe(export_name = "zo_regex_match_start")]
pub extern "C" fn _zo_regex_match_start(handle: i64) -> i64 {
  with_slot(&MATCH_CACHE, handle, || -1, |m| m.start)
}

/// Byte offset of the match's end (exclusive), or `-1`.
#[unsafe(export_name = "zo_regex_match_end")]
pub extern "C" fn _zo_regex_match_end(handle: i64) -> i64 {
  with_slot(&MATCH_CACHE, handle, || -1, |m| m.end)
}

/// Capture-group count, excluding the whole match.
#[unsafe(export_name = "zo_regex_match_group_count")]
pub extern "C" fn _zo_regex_match_group_count(handle: i64) -> i64 {
  with_slot(
    &MATCH_CACHE,
    handle,
    || 0,
    |m| (m.groups.len().saturating_sub(1)) as i64,
  )
}

/// Return the i-th capture's bytes as a `CBytes` value.
///
/// @note — `0` is the whole match; `1..N` are capture
/// groups. Empty bytes for out-of-range indices or non-
/// participating groups.
#[unsafe(export_name = "zo_regex_match_group")]
pub extern "C" fn _zo_regex_match_group(handle: i64, index: i64) -> CBytes {
  with_slot(&MATCH_CACHE, handle, CBytes::empty, |m| {
    let Ok(idx) = usize::try_from(index) else {
      return CBytes::empty();
    };

    m.groups
      .get(idx)
      .and_then(|g| g.as_deref())
      .map_or_else(CBytes::empty, |s| stage_cbytes(&SCRATCH, s.as_bytes()))
  })
}

/// Replace the first match in `haystack` with `replacement`.
///
/// @note — replacement supports `$0` (whole match) and
/// `$1`/`$2`/… (capture groups).
///
/// # Safety
///
/// `haystack` and `replacement` must be NUL-terminated UTF-8
/// or null.
#[unsafe(export_name = "zo_regex_replace")]
pub unsafe extern "C" fn _zo_regex_replace(
  handle: i64,
  haystack: *const c_char,
  replacement: *const c_char,
) -> CBytes {
  let haystack = unsafe { cstr_to_str(haystack) };
  let replacement = unsafe { cstr_to_str(replacement) };

  replace_via(handle, haystack, replacement, |re, h, r| {
    re.replacen(h, 1, r)
  })
}

/// Replace every match in `haystack` with `replacement`.
///
/// @note — replacement supports `$0` and `$1`/`$2`/…
/// backreferences.
///
/// # Safety
///
/// `haystack` and `replacement` must be NUL-terminated UTF-8
/// or null.
#[unsafe(export_name = "zo_regex_replace_all")]
pub unsafe extern "C" fn _zo_regex_replace_all(
  handle: i64,
  haystack: *const c_char,
  replacement: *const c_char,
) -> CBytes {
  let haystack = unsafe { cstr_to_str(haystack) };
  let replacement = unsafe { cstr_to_str(replacement) };

  replace_via(handle, haystack, replacement, |re, h, r| {
    re.replace_all(h, r)
  })
}

/// Split `haystack` on every match; return a zo `[]str`.
///
/// @note — layout matches `_zo_args` so codegen lifts this
/// with the same single-MOV path used for any
/// `pub ffi -> []str`. Empty array on invalid handle.
///
/// # Safety
///
/// `haystack` must be a NUL-terminated UTF-8 string or null.
#[unsafe(export_name = "zo_regex_split")]
pub unsafe extern "C" fn _zo_regex_split(
  handle: i64,
  haystack: *const c_char,
) -> *const u8 {
  let haystack = unsafe { cstr_to_str(haystack) };

  let pieces: Vec<*const u8> =
    with_slot(&REGEX_CACHE, handle, Vec::new, |re| {
      re.split(haystack)
        .map(|piece| crate::str::alloc_str(piece.as_bytes()))
        .collect()
    });

  crate::arr::alloc_ptr_array(&pieces)
}
