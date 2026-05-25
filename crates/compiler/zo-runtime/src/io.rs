//! Stdin readers backing `readln()` / `read()` in `std/io.zo`.
//!
//! Codegen calls these from `emit_io_readln` / `emit_io_read` —
//! one syscall worth of work each, plus a scan for `readln`'s
//! newline truncation. fd 0 is set non-blocking on first call;
//! `EAGAIN` routes through `park_current_on_read` (yield from
//! green tasks, `libc::poll` fallback otherwise).

use crate::net::{last_errno, park_current_on_read, park_read_or_classify};

/// Sized to match libc's default `BUFSIZ`.
const STDIN_BUFFER_SIZE: usize = 4096;

/// One-shot O_NONBLOCK install on fd 0. Idempotent and
/// atomic so concurrent first-callers can't race.
static STDIN_NONBLOCKING: std::sync::Once = std::sync::Once::new();

/// Mark fd 0 as `O_NONBLOCK`. Process-wide effect; any
/// other code reading stdin must accept `EAGAIN`.
fn ensure_stdin_nonblocking() {
  STDIN_NONBLOCKING.call_once(|| crate::net::set_nonblocking(0));
}

/// `buf[cursor..filled]` is the unread tail of the last
/// `read(0, ...)`; refilled when drained. `Mutex` because
/// `readln()` may be called from multiple threads and we
/// need a `static` (no thread-local with `const` init).
struct StdinBuffer {
  buf: [u8; STDIN_BUFFER_SIZE],
  cursor: usize,
  filled: usize,
}

impl StdinBuffer {
  const fn new() -> Self {
    Self {
      buf: [0; STDIN_BUFFER_SIZE],
      cursor: 0,
      filled: 0,
    }
  }
}

static STDIN: std::sync::Mutex<StdinBuffer> =
  std::sync::Mutex::new(StdinBuffer::new());

/// Read one line from stdin into `buf`, stopping at `\n`
/// (not included), EOF, or `max_len`. Returns the byte count
/// written (≥ 0) or `-errno` (< 0). EOF before any byte
/// returns 0; callers detect end-of-input by an empty line
/// followed by a second 0.
///
/// # Safety
///
/// `buf` must point at `max_len` writable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn zo_io_readln(
  buf: *mut u8,
  max_len: usize,
) -> isize {
  ensure_stdin_nonblocking();

  // SAFETY: caller guarantees `buf` points at `max_len`
  // writable bytes; no other reference to that region
  // exists for the lifetime of this call.
  let dst = unsafe { std::slice::from_raw_parts_mut(buf, max_len) };
  let mut state = STDIN.lock().unwrap();
  let mut written: usize = 0;

  'outer: loop {
    // Drain whatever's already buffered until we hit a
    // newline or fill the caller's destination.
    while state.cursor < state.filled && written < max_len {
      let byte = state.buf[state.cursor];
      state.cursor += 1;

      if byte == b'\n' {
        return written as isize;
      }

      dst[written] = byte;
      written += 1;
    }

    if written >= max_len {
      return written as isize;
    }

    // Buffer drained — refill. The buffer holds zero
    // useful bytes from this point until the next
    // successful read.
    state.cursor = 0;
    state.filled = 0;

    loop {
      let n = unsafe {
        libc::read(0, state.buf.as_mut_ptr() as *mut _, STDIN_BUFFER_SIZE)
      };

      if n > 0 {
        state.filled = n as usize;
        continue 'outer;
      }

      if n == 0 {
        // EOF — return whatever we've assembled.
        return written as isize;
      }

      let err = last_errno();

      if err != libc::EAGAIN && err != libc::EWOULDBLOCK {
        return -(err as isize);
      }

      // Drop the global STDIN lock across the park so a
      // sibling task on this scheduler thread can make
      // progress (and so the lock isn't held re-entrantly
      // across `yield_now`, which is UB on std::Mutex).
      drop(state);
      park_current_on_read(0);
      state = STDIN.lock().unwrap();

      // Another caller may have refilled the buffer
      // while we were parked. If so, drain it before
      // attempting another read.
      if state.filled > state.cursor {
        continue 'outer;
      }
    }
  }
}

/// Read up to `max_len` bytes from stdin into `buf`. Single
/// `read()` call — no loop, so input larger than `max_len`
/// is silently truncated. Returns the byte count (≥ 0) or
/// `-errno` (< 0). Multi-read accumulation is a follow-up.
///
/// # Safety
///
/// `buf` must point at `max_len` writable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn zo_io_read(
  buf: *mut u8,
  max_len: usize,
) -> isize {
  ensure_stdin_nonblocking();

  loop {
    // SAFETY: caller guarantees `buf` points at
    // `max_len` writable bytes for the call duration.
    let n = unsafe { libc::read(0, buf as *mut _, max_len) };

    if n >= 0 {
      return n;
    }

    if let Some(err) = park_read_or_classify(0, last_errno()) {
      return err as isize;
    }
  }
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
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn zo_args() -> *const u8 {
  *ARGS_ARRAY.get_or_init(|| {
    let str_ptrs: Vec<*const u8> = std::env::args_os()
      .skip(1)
      .map(|arg| crate::str::alloc_str(arg.as_encoded_bytes()))
      .collect();

    crate::arr::alloc_ptr_array(&str_ptrs) as usize
  }) as *const u8
}

/// Read directory entries at `path` into a heap `[]str`.
///
/// @note — `OsStr::from_bytes` skips a UTF-8 scan that
/// would reject directory names valid on the filesystem
/// but not in Rust's `&str`. Errors collapse to an empty
/// array — the codegen treats X0 as a zo `[]str` pointer
/// either way.
///
/// # Safety
///
/// `path` must point at a NUL-terminated byte run that
/// stays readable and unaliased for the duration of the
/// call.
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn zo_io_read_dir(path: *const u8) -> *const u8 {
  use std::os::unix::ffi::OsStrExt;

  if path.is_null() {
    return crate::arr::alloc_ptr_array(&[]);
  }

  let cstr = unsafe { std::ffi::CStr::from_ptr(path.cast()) };
  let os_path = std::ffi::OsStr::from_bytes(cstr.to_bytes());

  let Ok(iter) = std::fs::read_dir(os_path) else {
    return crate::arr::alloc_ptr_array(&[]);
  };

  let str_ptrs: Vec<*const u8> = iter
    .filter_map(Result::ok)
    .map(|entry| crate::str::alloc_str(entry.file_name().as_encoded_bytes()))
    .collect();

  crate::arr::alloc_ptr_array(&str_ptrs)
}
