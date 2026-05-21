//! sys — system-information primitives.
//!
//! The exported `_zo_sys_*` symbols back `core/sys/info.zo`'s
//! FFI surface. Implementation rides the `sysinfo` crate so
//! every Cranelift target (x86, arm64, Linux, macOS,
//! Windows) gets correct semantics without per-target FFI
//! shims — the same shape as `time.rs` riding `std::time`.
//!
//! Every export takes a flat scalar in / out (no structs
//! across the C boundary): callers refresh the global
//! `System` snapshot on first touch via `OnceLock`,
//! re-refresh per call so the values reflect the moment
//! the FFI fired. CPU% specifically needs two refreshes
//! spaced by `MINIMUM_CPU_UPDATE_INTERVAL`; the first call
//! after process start returns 0% and is the documented
//! sampling discipline.
//!
//! zo programs that never `load core::sys::info::*` link
//! none of these symbols — sysinfo's `System::new_all`
//! never runs, so the cost is bounded to the few KB of
//! sysinfo code statically linked into libzo_runtime.

use std::sync::{Mutex, OnceLock};
use sysinfo::System;

/// Global `System` snapshot — lazy on first FFI hit so
/// programs that don't import `core::sys::info` pay
/// nothing at runtime.
static SYSTEM: OnceLock<Mutex<System>> = OnceLock::new();

/// Refresh the global snapshot and run `f` against it.
/// `sysinfo::System` is not `Sync`, so callers serialize
/// through a `Mutex`; sysinfo calls are infrequent
/// (polled at most once per render frame) so contention
/// isn't a concern.
fn with_refreshed_system<F, R>(f: F) -> R
where
  F: FnOnce(&System) -> R,
{
  let cell = SYSTEM.get_or_init(|| Mutex::new(System::new_all()));
  let mut guard = cell.lock().expect("sysinfo system mutex poisoned");

  guard.refresh_all();

  f(&guard)
}

/// Current global CPU usage as a percentage in `0.0..=100.0`.
/// First call after process start returns 0.0 — sysinfo's
/// CPU sampler needs two refreshes
/// `MINIMUM_CPU_UPDATE_INTERVAL` apart to compute a delta.
/// Subsequent calls return the interval-averaged usage.
#[unsafe(export_name = "zo_sys_cpu_usage")]
pub extern "C-unwind" fn _zo_sys_cpu_usage() -> f32 {
  with_refreshed_system(|sys| sys.global_cpu_usage())
}

/// Number of logical CPUs the OS reports. Stable across
/// the process lifetime; doesn't need a refresh-pair.
#[unsafe(export_name = "zo_sys_cpu_count")]
pub extern "C-unwind" fn _zo_sys_cpu_count() -> i64 {
  with_refreshed_system(|sys| sys.cpus().len() as i64)
}

/// Total physical memory in bytes. Stable across the
/// process lifetime on every supported OS.
#[unsafe(export_name = "zo_sys_mem_total")]
pub extern "C-unwind" fn _zo_sys_mem_total() -> i64 {
  with_refreshed_system(|sys| sys.total_memory().min(i64::MAX as u64) as i64)
}

/// Used physical memory in bytes (excluding cache /
/// buffers on Linux). Updated on every call.
#[unsafe(export_name = "zo_sys_mem_used")]
pub extern "C-unwind" fn _zo_sys_mem_used() -> i64 {
  with_refreshed_system(|sys| sys.used_memory().min(i64::MAX as u64) as i64)
}

/// Available physical memory in bytes — what the OS would
/// hand out to a fresh allocator. Distinct from `total -
/// used` because `used` excludes reclaimable cache.
#[unsafe(export_name = "zo_sys_mem_available")]
pub extern "C-unwind" fn _zo_sys_mem_available() -> i64 {
  with_refreshed_system(|sys| {
    sys.available_memory().min(i64::MAX as u64) as i64
  })
}

/// Total swap space in bytes. Returns `0` when the OS
/// reports no swap configured.
#[unsafe(export_name = "zo_sys_swap_total")]
pub extern "C-unwind" fn _zo_sys_swap_total() -> i64 {
  with_refreshed_system(|sys| sys.total_swap().min(i64::MAX as u64) as i64)
}

/// Used swap space in bytes.
#[unsafe(export_name = "zo_sys_swap_used")]
pub extern "C-unwind" fn _zo_sys_swap_used() -> i64 {
  with_refreshed_system(|sys| sys.used_swap().min(i64::MAX as u64) as i64)
}

/// System uptime in whole seconds. Process-independent —
/// reflects how long the host has been running, not the
/// caller's process lifetime.
#[unsafe(export_name = "zo_sys_uptime_secs")]
pub extern "C-unwind" fn _zo_sys_uptime_secs() -> i64 {
  with_refreshed_system(|_| System::uptime().min(i64::MAX as u64) as i64)
}

/// Number of running processes the OS reports. Updated
/// per call.
#[unsafe(export_name = "zo_sys_proc_count")]
pub extern "C-unwind" fn _zo_sys_proc_count() -> i64 {
  with_refreshed_system(|sys| sys.processes().len() as i64)
}

/// One-minute load average. macOS / Linux return real
/// values; Windows has no native load-average and
/// returns 0.0.
#[unsafe(export_name = "zo_sys_load_avg_1m")]
pub extern "C-unwind" fn _zo_sys_load_avg_1m() -> f64 {
  System::load_average().one
}

/// Five-minute load average. Windows: 0.0.
#[unsafe(export_name = "zo_sys_load_avg_5m")]
pub extern "C-unwind" fn _zo_sys_load_avg_5m() -> f64 {
  System::load_average().five
}

/// Fifteen-minute load average. Windows: 0.0.
#[unsafe(export_name = "zo_sys_load_avg_15m")]
pub extern "C-unwind" fn _zo_sys_load_avg_15m() -> f64 {
  System::load_average().fifteen
}
