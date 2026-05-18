//! time — monotonic + wall-clock primitives.
//!
//! The exported `_zo_time_*` symbols back `core/time.zo`'s
//! FFI surface. Cross-target portability comes for free
//! from `std::time`, which already abstracts Windows
//! `QueryPerformanceCounter`, macOS mach time, Linux
//! `clock_gettime`, and WASM `performance.now()` per
//! target. zo's Cranelift backend can therefore reuse the
//! same dylib on every host without per-target FFI shims.
//!
//! `_zo_time_monotonic_ns` returns nanoseconds since a
//! fixed monotonic epoch captured at first call; the value
//! is unsuitable for cross-process comparison but is the
//! right primitive for `Instant::elapsed` / benchmark
//! timing on the zo side.

use std::sync::OnceLock;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Reference monotonic point. Initialised lazily at first
/// `_zo_time_monotonic_ns` so zo programs that never call
/// time pay nothing.
static MONOTONIC_EPOCH: OnceLock<Instant> = OnceLock::new();

/// Nanoseconds since the process's monotonic epoch.
/// Saturates at `i64::MAX` (~292 years) so the cast is
/// total; in practice no real run hits that.
#[unsafe(export_name = "zo_time_monotonic_ns")]
pub extern "C-unwind" fn _zo_time_monotonic_ns() -> i64 {
  let epoch = MONOTONIC_EPOCH.get_or_init(Instant::now);
  let nanos = Instant::now().saturating_duration_since(*epoch).as_nanos();

  nanos.min(i64::MAX as u128) as i64
}

/// Whole seconds of the current wall clock since the UNIX
/// epoch. Returns 0 if the system clock predates 1970
/// (theoretical — we don't crash on it).
#[unsafe(export_name = "zo_time_unix_secs")]
pub extern "C-unwind" fn _zo_time_unix_secs() -> i64 {
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map(|d| d.as_secs() as i64)
    .unwrap_or(0)
}

/// Sub-second nanosecond component of the current wall
/// clock (always in `0..1_000_000_000`). Paired with
/// `_zo_time_unix_secs` to form a full timestamp without
/// having to return a struct across the FFI boundary.
#[unsafe(export_name = "zo_time_unix_nanos")]
pub extern "C-unwind" fn _zo_time_unix_nanos() -> i64 {
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map(|d| d.subsec_nanos() as i64)
    .unwrap_or(0)
}

/// Block the current thread for `ns` nanoseconds. Negative
/// or zero is a no-op (matches Rust's `thread::sleep` for
/// `Duration::ZERO`). Splits ns into (secs, sub_ns) before
/// handing to `std::thread::sleep` so the kernel call uses
/// the cheaper second-granularity path for long sleeps.
#[unsafe(export_name = "zo_time_sleep_ns")]
pub extern "C-unwind" fn _zo_time_sleep_ns(ns: i64) {
  if ns <= 0 {
    return;
  }

  let nanos = ns as u64;
  let secs = nanos / 1_000_000_000;
  let sub = (nanos % 1_000_000_000) as u32;

  thread::sleep(Duration::new(secs, sub));
}
