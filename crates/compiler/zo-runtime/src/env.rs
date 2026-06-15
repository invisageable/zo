//! Environment variables + process working directory.

use zo_c_abi::{CBytes, cstr_to_str, stage_cbytes};

use std::cell::RefCell;
use std::os::raw::c_char;

thread_local! {
  static SCRATCH: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

/// Read environment variable `name`; empty bytes on miss.
///
/// # Safety
///
/// `name` must be a NUL-terminated UTF-8 string or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zo_env_get(name: *const c_char) -> CBytes {
  let name = unsafe { cstr_to_str(name) };

  std::env::var(name)
    .map(|value| stage_cbytes(&SCRATCH, value.as_bytes()))
    .unwrap_or_else(|_| CBytes::empty())
}

/// Set environment variable `name` to `value`.
///
/// @note — returns `1` on success, `0` when `name` is empty
/// or contains `=` (POSIX `setenv` would `EINVAL`). The
/// underlying `std::env::set_var` is process-wide and not
/// thread-safe — caller must serialise across tasks.
///
/// # Safety
///
/// `name` and `value` must be NUL-terminated UTF-8 strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zo_env_set(
  name: *const c_char,
  value: *const c_char,
) -> i64 {
  let name = unsafe { cstr_to_str(name) };
  let value = unsafe { cstr_to_str(value) };

  if name.is_empty() || name.contains('=') {
    return 0;
  }

  unsafe { std::env::set_var(name, value) };
  1
}

/// Unset environment variable `name`; idempotent.
///
/// @note — returns `1` on success, `0` when `name` is empty
/// or contains `=` (POSIX `unsetenv` would `EINVAL`).
/// Removing an absent variable still succeeds.
///
/// # Safety
///
/// `name` must be a NUL-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zo_env_remove(name: *const c_char) -> i64 {
  let name = unsafe { cstr_to_str(name) };

  if name.is_empty() || name.contains('=') {
    return 0;
  }

  unsafe { std::env::remove_var(name) };
  1
}

/// All env vars as a heap `[]str` of `"KEY=VALUE"` entries.
#[unsafe(no_mangle)]
pub extern "C" fn zo_env_vars() -> *const u8 {
  let pieces: Vec<*const u8> = std::env::vars()
    .map(|(key, value)| {
      crate::str::alloc_str(format!("{key}={value}").as_bytes())
    })
    .collect();

  crate::arr::alloc_ptr_array(&pieces)
}

/// Process working directory; empty bytes on error.
#[unsafe(no_mangle)]
pub extern "C" fn zo_env_current_dir() -> CBytes {
  std::env::current_dir()
    .ok()
    .and_then(|p| p.to_str().map(|s| stage_cbytes(&SCRATCH, s.as_bytes())))
    .unwrap_or_else(CBytes::empty)
}

/// Set the process working directory. `0` on success,
/// `-errno` on failure. The path uses the zo-str (`*const u8`)
/// contract shared with `io` — no `CStr` allocation.
///
/// # Safety
///
/// `path` must be a valid zo str header.
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn zo_env_set_current_dir(path: *const u8) -> i64 {
  let Some(p) = (unsafe { crate::io::path_arg(path) }) else {
    return -22; // EINVAL
  };

  match std::env::set_current_dir(&p) {
    Ok(()) => 0,
    Err(e) => crate::io::errno(e),
  }
}

/// The OS temp directory ($TMPDIR / platform default); empty
/// bytes when its path isn't valid UTF-8.
#[unsafe(no_mangle)]
pub extern "C" fn zo_env_temp_dir() -> CBytes {
  std::env::temp_dir()
    .to_str()
    .map(|s| stage_cbytes(&SCRATCH, s.as_bytes()))
    .unwrap_or_else(CBytes::empty)
}
