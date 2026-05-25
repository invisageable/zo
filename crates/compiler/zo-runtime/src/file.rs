//! POSIX file operations — `core/file.zo` FFI backing.
//!
//! Thin wrappers around `open`, `close`, `read`, `write`,
//! `lseek`, `stat`, `mkdir`, `rename`. Error convention:
//! negative return = -errno.

use crate::net::last_errno;
use crate::str::{alloc_str, str_bytes};

/// `file::open(path, flags, mode)` — POSIX `open(2)`.
/// Returns fd on success, -errno on failure.
///
/// # Safety
///
/// `path` must be a NUL-terminated C string pointer.
#[unsafe(export_name = "zo_file_open")]
pub unsafe extern "C-unwind" fn _zo_file_open(
  path: *const u8,
  flags: i32,
  mode: i32,
) -> i32 {
  let fd = unsafe { libc::open(path.cast(), flags, mode as libc::c_uint) };

  if fd < 0 { -(last_errno() as i32) } else { fd }
}

/// `file::close(fd)` — POSIX `close(2)`.
/// Returns 0 on success, -errno on failure.
///
/// # Safety
///
/// `fd` must be a valid open file descriptor.
#[unsafe(export_name = "zo_file_close")]
pub unsafe extern "C-unwind" fn _zo_file_close(fd: i32) -> i32 {
  let result = unsafe { libc::close(fd) };

  if result < 0 {
    -(last_errno() as i32)
  } else {
    0
  }
}

/// `file::read(fd, buf, len)` — POSIX `read(2)`.
/// Returns bytes read on success, -errno on failure.
///
/// # Safety
///
/// `buf` must point at `len` writable bytes.
#[unsafe(export_name = "zo_file_read")]
pub unsafe extern "C-unwind" fn _zo_file_read(
  fd: i32,
  buf: *mut u8,
  len: usize,
) -> isize {
  let n = unsafe { libc::read(fd, buf.cast(), len) };

  if n < 0 { -(last_errno() as isize) } else { n }
}

/// `file::write(fd, buf, len)` — POSIX `write(2)`.
/// Returns bytes written on success, -errno on failure.
///
/// # Safety
///
/// `buf` must point at `len` readable bytes.
#[unsafe(export_name = "zo_file_write")]
pub unsafe extern "C-unwind" fn _zo_file_write(
  fd: i32,
  buf: *const u8,
  len: usize,
) -> isize {
  let n = unsafe { libc::write(fd, buf.cast(), len) };

  if n < 0 { -(last_errno() as isize) } else { n }
}

/// `file::seek(fd, offset, whence)` — POSIX `lseek(2)`.
/// Returns new offset on success, -errno on failure.
///
/// # Safety
///
/// `fd` must be a valid open file descriptor.
#[unsafe(export_name = "zo_file_seek")]
pub unsafe extern "C-unwind" fn _zo_file_seek(
  fd: i32,
  offset: i32,
  whence: i32,
) -> i32 {
  let pos = unsafe { libc::lseek(fd, offset as libc::off_t, whence) };

  if pos < 0 {
    -(last_errno() as i32)
  } else {
    pos as i32
  }
}

/// `file::stat_size(path)` — file size in bytes via
/// `stat(2)`. Returns -errno on failure.
///
/// # Safety
///
/// `path` must be a NUL-terminated C string pointer.
#[unsafe(export_name = "zo_file_stat_size")]
pub unsafe extern "C-unwind" fn _zo_file_stat_size(path: *const u8) -> i32 {
  let mut st: libc::stat = unsafe { std::mem::zeroed() };
  let result = unsafe { libc::stat(path.cast(), &mut st) };

  if result < 0 {
    -(last_errno() as i32)
  } else {
    st.st_size as i32
  }
}

/// `file::stat_mtime(path)` — last modification time as
/// Unix epoch seconds via `stat(2)`. Returns -errno on
/// failure.
///
/// # Safety
///
/// `path` must be a NUL-terminated C string pointer.
#[unsafe(export_name = "zo_file_stat_mtime")]
pub unsafe extern "C-unwind" fn _zo_file_stat_mtime(path: *const u8) -> i32 {
  let mut st: libc::stat = unsafe { std::mem::zeroed() };
  let result = unsafe { libc::stat(path.cast(), &mut st) };

  if result < 0 {
    -(last_errno() as i32)
  } else {
    st.st_mtime as i32
  }
}

/// `file::mkdir(path, mode)` — POSIX `mkdir(2)`.
/// Returns 0 on success, -errno on failure.
///
/// # Safety
///
/// `path` must be a NUL-terminated C string pointer.
#[unsafe(export_name = "zo_file_mkdir")]
pub unsafe extern "C-unwind" fn _zo_file_mkdir(
  path: *const u8,
  mode: i32,
) -> i32 {
  let result = unsafe { libc::mkdir(path.cast(), mode as libc::mode_t) };

  if result < 0 {
    -(last_errno() as i32)
  } else {
    0
  }
}

/// `file::rename(old, new)` — POSIX `rename(2)`.
/// Returns 0 on success, -errno on failure.
///
/// # Safety
///
/// Both pointers must be NUL-terminated C string pointers.
#[unsafe(export_name = "zo_file_rename")]
pub unsafe extern "C-unwind" fn _zo_file_rename(
  old: *const u8,
  new: *const u8,
) -> i32 {
  let result = unsafe { libc::rename(old.cast(), new.cast()) };

  if result < 0 {
    -(last_errno() as i32)
  } else {
    0
  }
}

/// Read the full contents of `fd` into a fresh zo `str`.
/// Used by `File::read(self, max_len)` — allocates a
/// scratch buffer, reads up to `max_len` bytes, then
/// copies the populated portion into a heap zo `str`.
///
/// Returns the zo `str` pointer in the low 64 bits and
/// a negative errno as a separate return value on failure.
///
/// # Safety
///
/// `fd` must be a valid open file descriptor.
#[unsafe(export_name = "zo_file_read_str")]
pub unsafe extern "C-unwind" fn _zo_file_read_str(
  fd: i32,
  max_len: i32,
) -> *const u8 {
  let max_len = max_len as usize;
  let mut buf = vec![0u8; max_len];
  let n = unsafe { libc::read(fd, buf.as_mut_ptr().cast(), max_len) };

  if n < 0 {
    return std::ptr::null();
  }

  alloc_str(&buf[..n as usize])
}

/// Write zo `str` payload to `fd`. Skips the 8-byte
/// length header so the kernel sees raw bytes.
///
/// Returns bytes written on success, -errno on failure.
///
/// # Safety
///
/// `s` must be a valid zo str header. `fd` must be open
/// for writing.
#[unsafe(export_name = "zo_file_write_str")]
pub unsafe extern "C-unwind" fn _zo_file_write_str(
  fd: i32,
  s: *const u8,
) -> isize {
  let bytes = unsafe { str_bytes(s) };
  let n = unsafe { libc::write(fd, bytes.as_ptr().cast(), bytes.len()) };

  if n < 0 { -(last_errno() as isize) } else { n }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::ffi::CString;

  fn tmp_path(name: &str) -> CString {
    let dir = std::env::temp_dir();
    let path = dir.join(format!("zo_file_test_{name}"));

    CString::new(path.to_str().unwrap()).unwrap()
  }

  #[test]
  fn open_write_read_close_roundtrip() {
    let path = tmp_path("roundtrip");

    unsafe {
      let fd = _zo_file_open(
        path.as_ptr().cast(),
        libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC,
        0o644,
      );

      assert!(fd >= 0, "open for write failed: {fd}");

      let data = b"hello file";
      let written = _zo_file_write(fd, data.as_ptr(), data.len());

      assert_eq!(written, data.len() as isize);
      assert_eq!(_zo_file_close(fd), 0);

      let fd = _zo_file_open(path.as_ptr().cast(), libc::O_RDONLY, 0);

      assert!(fd >= 0, "open for read failed: {fd}");

      let mut buf = [0u8; 64];
      let n = _zo_file_read(fd, buf.as_mut_ptr(), buf.len());

      assert_eq!(n, data.len() as isize);
      assert_eq!(&buf[..n as usize], data);
      assert_eq!(_zo_file_close(fd), 0);

      libc::unlink(path.as_ptr().cast());
    }
  }

  #[test]
  fn seek_and_partial_read() {
    let path = tmp_path("seek");

    unsafe {
      let fd = _zo_file_open(
        path.as_ptr().cast(),
        libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC,
        0o644,
      );

      let data = b"abcdefghij";

      _zo_file_write(fd, data.as_ptr(), data.len());
      _zo_file_close(fd);

      let fd = _zo_file_open(path.as_ptr().cast(), libc::O_RDONLY, 0);

      assert!(fd >= 0);

      let pos = _zo_file_seek(fd, 5, libc::SEEK_SET);

      assert_eq!(pos, 5);

      let mut buf = [0u8; 5];
      let n = _zo_file_read(fd, buf.as_mut_ptr(), 5);

      assert_eq!(n, 5);
      assert_eq!(&buf, b"fghij");

      _zo_file_close(fd);
      libc::unlink(path.as_ptr().cast());
    }
  }

  #[test]
  fn stat_size_matches_written_bytes() {
    let path = tmp_path("stat");

    unsafe {
      let fd = _zo_file_open(
        path.as_ptr().cast(),
        libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC,
        0o644,
      );

      let data = b"twelve bytes";

      _zo_file_write(fd, data.as_ptr(), data.len());
      _zo_file_close(fd);

      let size = _zo_file_stat_size(path.as_ptr().cast());

      assert_eq!(size, data.len() as i32);

      libc::unlink(path.as_ptr().cast());
    }
  }

  #[test]
  fn mkdir_and_rename() {
    let dir = tmp_path("mkdir_test");
    let dir2 = tmp_path("mkdir_renamed");

    unsafe {
      let result = _zo_file_mkdir(dir.as_ptr().cast(), 0o755);

      assert_eq!(result, 0);

      let result = _zo_file_rename(dir.as_ptr().cast(), dir2.as_ptr().cast());

      assert_eq!(result, 0);

      libc::rmdir(dir2.as_ptr().cast());
    }
  }

  #[test]
  fn open_nonexistent_returns_negative_errno() {
    let path = tmp_path("nonexistent_file_that_does_not_exist");

    unsafe {
      let fd = _zo_file_open(path.as_ptr().cast(), libc::O_RDONLY, 0);

      assert!(fd < 0, "expected negative errno, got {fd}");
      assert_eq!(fd, -libc::ENOENT);
    }
  }
}
