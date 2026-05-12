//! zo-provider-sqlite — runtime backing the user-facing
//! `compiler-lib/std/sqlite.zo`. The user opens a database
//! handle, runs DDL/DML, and reads single-int query
//! results; this crate forwards each call through
//! `rusqlite` (which embeds `bundled` SQLite).
//!
//! Cross-thread contract: `rusqlite::Connection` is
//! `!Send`, but the registry is wrapped in `Mutex` so an
//! occasional cross-thread borrow doesn't panic. Heavy
//! parallel users should open one DB per worker.
//!
//! Handle protocol: every `__zo_sqlite_open` returns a
//! 1-based `i64` index into a global `Vec<Option<Connection>>`.
//! `0` is reserved for "open failed" — match `_open`'s
//! return against `0` in zo to detect errors. `_close`
//! drops the slot but doesn't compact, so existing
//! handles stay stable.

use std::ffi::CStr;
use std::os::raw::c_char;
use std::sync::Mutex;

use rusqlite::Connection;

/// Global handle registry. `LazyLock` keeps the
/// initialization branch out of every call site. Vec is
/// indexed by `(handle - 1)`; index 0 is reserved for the
/// "open failed" sentinel returned to zo.
static REGISTRY: Mutex<Vec<Option<Connection>>> = Mutex::new(Vec::new());

/// zo-side `int` is i64, matching the AAPCS GP register.
type ZoHandle = i64;

/// Read a `*const c_char` C string passed from zo's
/// `c_str(...)` helper. Returns an owned `String` since
/// `rusqlite` borrows are short and the cost is negligible
/// for typical SQL strings (< 1 KiB).
unsafe fn read_c_str(ptr: *const c_char) -> Option<String> {
  if ptr.is_null() {
    return None;
  }

  unsafe { CStr::from_ptr(ptr) }
    .to_str()
    .ok()
    .map(str::to_owned)
}

/// `__zo_sqlite_open(path: int) -> int`.
/// Returns a positive handle on success, `0` on failure
/// (file open, permissions, malformed path).
#[unsafe(no_mangle)]
pub extern "C" fn __zo_sqlite_open(path: *const c_char) -> ZoHandle {
  let Some(path) = (unsafe { read_c_str(path) }) else {
    return 0;
  };

  let Ok(conn) = Connection::open(&path) else {
    return 0;
  };

  let mut reg = REGISTRY.lock().unwrap();

  reg.push(Some(conn));
  reg.len() as ZoHandle
}

/// `__zo_sqlite_close(handle: int)`. Idempotent — a
/// handle that's already been closed (or was never valid)
/// is a no-op. The slot is set to `None` so subsequent
/// `_exec` / `_query_int` calls on the stale handle
/// return an error code.
#[unsafe(no_mangle)]
pub extern "C" fn __zo_sqlite_close(handle: ZoHandle) {
  let mut reg = REGISTRY.lock().unwrap();
  let idx = (handle - 1) as usize;

  if let Some(slot) = reg.get_mut(idx) {
    *slot = None;
  }
}

/// `__zo_sqlite_exec(handle: int, sql: int) -> int`.
/// Runs DDL/DML (`CREATE TABLE`, `INSERT`, `UPDATE`,
/// `DELETE`). Returns `0` on success, non-zero on
/// failure. SQL errors are absorbed — for a real client a
/// future API would surface the error string; today's
/// scoreboard demo only needs success/fail.
#[unsafe(no_mangle)]
pub extern "C" fn __zo_sqlite_exec(
  handle: ZoHandle,
  sql: *const c_char,
) -> ZoHandle {
  let Some(sql) = (unsafe { read_c_str(sql) }) else {
    return 1;
  };
  let reg = REGISTRY.lock().unwrap();
  let idx = (handle - 1) as usize;
  let Some(Some(conn)) = reg.get(idx) else {
    return 1;
  };

  match conn.execute_batch(&sql) {
    Ok(()) => 0,
    Err(_) => 1,
  }
}

/// `__zo_sqlite_query_int(handle: int, sql: int) -> int`.
/// Runs a `SELECT` and returns the first column of the
/// first row as `int` (i64). Returns `0` when no rows or
/// on any error — caller can't distinguish a real `0`
/// from an error today, which is fine for aggregates like
/// `COUNT(*)` / `MAX(score)` where the demo treats `0` as
/// "no data".
#[unsafe(no_mangle)]
pub extern "C" fn __zo_sqlite_query_int(
  handle: ZoHandle,
  sql: *const c_char,
) -> ZoHandle {
  let Some(sql) = (unsafe { read_c_str(sql) }) else {
    return 0;
  };
  let reg = REGISTRY.lock().unwrap();
  let idx = (handle - 1) as usize;
  let Some(Some(conn)) = reg.get(idx) else {
    return 0;
  };

  conn
    .query_row(&sql, [], |row| row.get::<_, i64>(0))
    .unwrap_or(0)
}
