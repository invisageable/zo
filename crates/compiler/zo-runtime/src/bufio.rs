//! Buffered I/O — `core/io.zo` BufferedReader/BufferedWriter
//! FFI backing.
//!
//! Each handle is a leaked `Box` pointer cast to `i64`.
//! Same opaque-handle pattern as `regex.rs`. The caller
//! must call `free` to reclaim the allocation.

use crate::str::alloc_str;

const DEFAULT_BUF_SIZE: usize = 8192;

/// Internal reader state — owns the buffer and tracks
/// how much has been filled/consumed.
struct ReaderState {
  fd: i32,
  buf: Vec<u8>,
  cursor: usize,
  filled: usize,
}

impl ReaderState {
  fn new(fd: i32, buf_size: usize) -> Self {
    let size = if buf_size > 0 {
      buf_size
    } else {
      DEFAULT_BUF_SIZE
    };

    Self {
      fd,
      buf: vec![0u8; size],
      cursor: 0,
      filled: 0,
    }
  }

  /// Refill the buffer from the fd. Returns bytes read
  /// or -errno on failure.
  fn refill(&mut self) -> isize {
    self.cursor = 0;
    self.filled = 0;

    let n = unsafe {
      libc::read(self.fd, self.buf.as_mut_ptr().cast(), self.buf.len())
    };

    if n > 0 {
      self.filled = n as usize;
    }

    n
  }

  /// Read one line (up to `\n`, not included). Returns
  /// a zo str pointer, or null on EOF/error.
  fn read_line(&mut self) -> *const u8 {
    let mut line = Vec::new();

    loop {
      while self.cursor < self.filled {
        let byte = self.buf[self.cursor];

        self.cursor += 1;

        if byte == b'\n' {
          return alloc_str(&line);
        }

        if byte != b'\r' {
          line.push(byte);
        }
      }

      let n = self.refill();

      if n <= 0 {
        if line.is_empty() {
          return std::ptr::null();
        }

        return alloc_str(&line);
      }
    }
  }

  /// Read up to `max` bytes. Returns a zo str pointer,
  /// or null on EOF/error.
  fn read(&mut self, max: usize) -> *const u8 {
    if self.cursor >= self.filled {
      let n = self.refill();

      if n <= 0 {
        return std::ptr::null();
      }
    }

    let available = self.filled - self.cursor;
    let take = available.min(max);
    let slice = &self.buf[self.cursor..self.cursor + take];

    self.cursor += take;

    alloc_str(slice)
  }
}

/// Internal writer state.
struct WriterState {
  fd: i32,
  buf: Vec<u8>,
}

impl WriterState {
  fn new(fd: i32, buf_size: usize) -> Self {
    let size = if buf_size > 0 {
      buf_size
    } else {
      DEFAULT_BUF_SIZE
    };

    Self {
      fd,
      buf: Vec::with_capacity(size),
    }
  }

  fn write(&mut self, data: &[u8]) -> isize {
    let cap = self.buf.capacity();

    for chunk in data.chunks(cap) {
      self.buf.extend_from_slice(chunk);

      if self.buf.len() >= cap {
        let n = self.flush_inner();

        if n < 0 {
          return n;
        }
      }
    }

    data.len() as isize
  }

  fn flush_inner(&mut self) -> isize {
    if self.buf.is_empty() {
      return 0;
    }

    let mut written = 0;

    while written < self.buf.len() {
      let n = unsafe {
        libc::write(
          self.fd,
          self.buf[written..].as_ptr().cast(),
          self.buf.len() - written,
        )
      };

      if n < 0 {
        let err = crate::net::last_errno();

        self.buf.drain(..written);

        return -(err as isize);
      }

      written += n as usize;
    }

    self.buf.clear();
    written as isize
  }
}

// -- Reader FFI --

/// # Safety
///
/// `fd` must be a valid open file descriptor.
#[unsafe(export_name = "zo_bufio_reader_new")]
pub unsafe extern "C-unwind" fn _zo_bufio_reader_new(
  fd: i32,
  buf_size: i32,
) -> i64 {
  let state = Box::new(ReaderState::new(fd, buf_size as usize));

  Box::into_raw(state) as i64
}

/// # Safety
///
/// `handle` must be a live reader from `_zo_bufio_reader_new`.
#[unsafe(export_name = "zo_bufio_reader_read_line")]
pub unsafe extern "C-unwind" fn _zo_bufio_reader_read_line(
  handle: i64,
) -> *const u8 {
  let state = unsafe { &mut *(handle as *mut ReaderState) };

  state.read_line()
}

/// # Safety
///
/// `handle` must be a live reader from `_zo_bufio_reader_new`.
#[unsafe(export_name = "zo_bufio_reader_read")]
pub unsafe extern "C-unwind" fn _zo_bufio_reader_read(
  handle: i64,
  max: i32,
) -> *const u8 {
  let state = unsafe { &mut *(handle as *mut ReaderState) };

  state.read(max as usize)
}

/// # Safety
///
/// `handle` must be a live reader (or 0 for no-op).
#[unsafe(export_name = "zo_bufio_reader_free")]
pub unsafe extern "C-unwind" fn _zo_bufio_reader_free(handle: i64) {
  if handle != 0 {
    drop(unsafe { Box::from_raw(handle as *mut ReaderState) });
  }
}

// -- Writer FFI --

/// # Safety
///
/// `fd` must be a valid open file descriptor.
#[unsafe(export_name = "zo_bufio_writer_new")]
pub unsafe extern "C-unwind" fn _zo_bufio_writer_new(
  fd: i32,
  buf_size: i32,
) -> i64 {
  let state = Box::new(WriterState::new(fd, buf_size as usize));

  Box::into_raw(state) as i64
}

/// # Safety
///
/// `handle` must be a live writer. `data` must point at
/// `len` readable bytes.
#[unsafe(export_name = "zo_bufio_writer_write")]
pub unsafe extern "C-unwind" fn _zo_bufio_writer_write(
  handle: i64,
  data: *const u8,
  len: usize,
) -> isize {
  let state = unsafe { &mut *(handle as *mut WriterState) };
  let bytes = unsafe { std::slice::from_raw_parts(data, len) };

  state.write(bytes)
}

/// # Safety
///
/// `handle` must be a live writer. `s` must be a valid
/// zo str header.
#[unsafe(export_name = "zo_bufio_writer_write_str")]
pub unsafe extern "C-unwind" fn _zo_bufio_writer_write_str(
  handle: i64,
  s: *const u8,
) -> isize {
  let state = unsafe { &mut *(handle as *mut WriterState) };
  let bytes = unsafe { crate::str::str_bytes(s) };

  state.write(bytes)
}

/// # Safety
///
/// `handle` must be a live writer.
#[unsafe(export_name = "zo_bufio_writer_flush")]
pub unsafe extern "C-unwind" fn _zo_bufio_writer_flush(handle: i64) -> i32 {
  let state = unsafe { &mut *(handle as *mut WriterState) };
  let n = state.flush_inner();

  if n < 0 { n as i32 } else { 0 }
}

/// # Safety
///
/// `handle` must be a live writer (or 0 for no-op).
#[unsafe(export_name = "zo_bufio_writer_free")]
pub unsafe extern "C-unwind" fn _zo_bufio_writer_free(handle: i64) {
  if handle != 0 {
    let mut state = unsafe { Box::from_raw(handle as *mut WriterState) };

    let _ = state.flush_inner();

    drop(state);
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::ffi::CString;

  fn tmp_path(name: &str) -> CString {
    let dir = std::env::temp_dir();
    let path = dir.join(format!("zo_bufio_test_{name}"));

    CString::new(path.to_str().unwrap()).unwrap()
  }

  #[test]
  fn buffered_write_then_read_lines() {
    let path = tmp_path("lines");

    unsafe {
      let fd = libc::open(
        path.as_ptr(),
        libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC,
        0o644u32 as libc::c_uint,
      );

      assert!(fd >= 0);

      let wh = _zo_bufio_writer_new(fd, 64);

      for i in 0..10 {
        let line = format!("line {i}\n");

        _zo_bufio_writer_write(wh, line.as_ptr(), line.len());
      }

      _zo_bufio_writer_free(wh);
      libc::close(fd);

      let fd = libc::open(path.as_ptr(), libc::O_RDONLY, 0);

      assert!(fd >= 0);

      let rh = _zo_bufio_reader_new(fd, 32);
      let mut count = 0;

      loop {
        let ptr = _zo_bufio_reader_read_line(rh);

        if ptr.is_null() {
          break;
        }

        count += 1;
      }

      _zo_bufio_reader_free(rh);
      libc::close(fd);

      assert_eq!(count, 10);

      libc::unlink(path.as_ptr());
    }
  }

  #[test]
  fn buffered_read_chunks() {
    let path = tmp_path("chunks");

    unsafe {
      let fd = libc::open(
        path.as_ptr(),
        libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC,
        0o644u32 as libc::c_uint,
      );

      let data = b"abcdefghijklmnopqrstuvwxyz";

      libc::write(fd, data.as_ptr().cast(), data.len());
      libc::close(fd);

      let fd = libc::open(path.as_ptr(), libc::O_RDONLY, 0);
      let rh = _zo_bufio_reader_new(fd, 8);

      let ptr1 = _zo_bufio_reader_read(rh, 5);

      assert!(!ptr1.is_null());

      let bytes1 = crate::str::str_bytes(ptr1);

      assert_eq!(bytes1, b"abcde");

      let ptr2 = _zo_bufio_reader_read(rh, 3);
      let bytes2 = crate::str::str_bytes(ptr2);

      assert_eq!(bytes2, b"fgh");

      _zo_bufio_reader_free(rh);
      libc::close(fd);
      libc::unlink(path.as_ptr());
    }
  }
}
