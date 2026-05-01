//! Stdin readers backing `readln()` / `read()` in `std/io.zo`.
//!
//! Codegen calls these from `emit_io_readln` / `emit_io_read` —
//! one syscall worth of work each, plus a scan for `readln`'s
//! newline truncation. Returning the byte count (or a negative
//! error) keeps the calling-convention symmetry with the
//! existing `read_file`-style inline syscalls.

/// Sized to match libc's default `BUFSIZ`.
const STDIN_BUFFER_SIZE: usize = 4096;

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
#[unsafe(export_name = "zo_io_readln")]
pub unsafe extern "C-unwind" fn _zo_io_readln(
  buf: *mut u8,
  max_len: usize,
) -> isize {
  // SAFETY: caller guarantees `buf` points at `max_len`
  // writable bytes; no other reference to that region
  // exists for the lifetime of this call.
  let dst = unsafe { std::slice::from_raw_parts_mut(buf, max_len) };
  let mut state = STDIN.lock().unwrap();
  let mut written: usize = 0;

  loop {
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

    // Buffer empty — refill with a single syscall. EOF means
    // we hand back whatever we already wrote (could be 0).
    state.cursor = 0;
    state.filled = 0;

    let n = unsafe {
      libc::read(0, state.buf.as_mut_ptr() as *mut _, STDIN_BUFFER_SIZE)
    };

    if n < 0 {
      return n as isize;
    }

    if n == 0 {
      return written as isize;
    }

    state.filled = n as usize;
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
