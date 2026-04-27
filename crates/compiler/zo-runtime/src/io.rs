//! Stdin readers backing `readln()` / `read()` in `std/io.zo`.
//!
//! Codegen calls these from `emit_io_readln` / `emit_io_read` —
//! one syscall worth of work each, plus a scan for `readln`'s
//! newline truncation. Returning the byte count (or a negative
//! error) keeps the calling-convention symmetry with the
//! existing `read_file`-style inline syscalls.

/// Read up to `max_len` bytes from stdin into `buf`, then
/// truncate the returned length at the first `\n`. Returns the
/// truncated byte count (≥ 0) on success, or `-errno` (< 0) on
/// read error.
///
/// # Safety
///
/// `buf` must point at `max_len` writable bytes.
#[unsafe(export_name = "zo_io_readln")]
pub unsafe extern "C-unwind" fn _zo_io_readln(
  buf: *mut u8,
  max_len: usize,
) -> isize {
  let n = unsafe { libc::read(0, buf as *mut _, max_len) };

  if n < 0 {
    return n as isize;
  }

  let len = n as usize;
  let bytes = unsafe { std::slice::from_raw_parts(buf, len) };

  bytes.iter().position(|&b| b == b'\n').unwrap_or(len) as isize
}

/// Read up to `max_len` bytes from stdin into `buf`. Single
/// `read()` call — no loop, so input larger than `max_len`
/// is silently truncated. Returns the byte count (≥ 0) or
/// `-errno` (< 0). Multi-read accumulation is a follow-up.
///
/// # Safety
///
/// `buf` must point at `max_len` writable bytes.
#[unsafe(export_name = "zo_io_read")]
pub unsafe extern "C-unwind" fn _zo_io_read(
  buf: *mut u8,
  max_len: usize,
) -> isize {
  unsafe { libc::read(0, buf as *mut _, max_len) }
}

/// Memoized argv array — argv never changes during the
/// process lifetime, so multiple `args()` calls return the
/// same heap-leaked pointer instead of leaking a fresh
/// array each time.
static ARGS_ARRAY: std::sync::OnceLock<usize> = std::sync::OnceLock::new();

/// Build (or return the memoized) zo `[]str` from the
/// process's argv, skipping argv[0]. Each element is a
/// heap-leaked zo str header (`[len:u64][bytes][null]`);
/// the array itself is laid out as
/// `[len:u64][cap:u64][str_ptr0:u64]...[str_ptrN:u64]`,
/// matching the codegen's runtime array shape.
///
/// # Safety
///
/// `unsafe` only because `extern "C-unwind"` requires it;
/// the body uses safe `std::env::args_os`.
#[unsafe(export_name = "zo_args")]
pub unsafe extern "C-unwind" fn _zo_args() -> *const u8 {
  *ARGS_ARRAY.get_or_init(|| {
    let str_ptrs: Vec<*const u8> = std::env::args_os()
      .skip(1)
      .map(|arg| crate::str::alloc_str(arg.as_encoded_bytes()))
      .collect();

    crate::arr::alloc_ptr_array(&str_ptrs) as usize
  }) as *const u8
}
