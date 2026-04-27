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
