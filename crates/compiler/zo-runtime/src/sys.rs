//! sys — system-information primitives.
//!
//! Backs `core::sys::info`'s FFI surface with direct platform
//! syscalls — libc + mach on macOS, the `sysinfo(2)` syscall
//! on Linux — so `libzo_runtime` pulls in no Foundation /
//! Obj-C dependency tree. A zo binary that never queries
//! system info links only `libSystem`. Memory values are
//! bytes, durations whole seconds, CPU usage a `0.0..=100.0`
//! percentage. The first `cpu_usage` call after process start
//! returns `0.0` — usage is an interval delta and the first
//! call has no prior sample to diff against.

#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::sync::atomic::{AtomicU64, Ordering};

/// Previous cumulative busy CPU ticks, for the `cpu_usage`
/// interval delta. `0` until the first sample lands.
#[cfg(any(target_os = "macos", target_os = "linux"))]
static CPU_PREV_BUSY: AtomicU64 = AtomicU64::new(0);

/// Previous cumulative total CPU ticks, paired with
/// `CPU_PREV_BUSY`.
#[cfg(any(target_os = "macos", target_os = "linux"))]
static CPU_PREV_TOTAL: AtomicU64 = AtomicU64::new(0);

/// Turn a pair of cumulative (busy, total) CPU tick counts
/// into an interval-averaged usage percentage, storing the
/// sample for the next call. The first call returns `0.0`
/// because there is no prior interval to diff against.
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn cpu_usage_delta(busy: u64, total: u64) -> f32 {
  let prev_busy = CPU_PREV_BUSY.swap(busy, Ordering::Relaxed);
  let prev_total = CPU_PREV_TOTAL.swap(total, Ordering::Relaxed);

  if prev_total == 0 || total <= prev_total {
    return 0.0;
  }

  let busy_delta = busy.saturating_sub(prev_busy) as f32;
  let total_delta = (total - prev_total) as f32;

  (busy_delta / total_delta * 100.0).clamp(0.0, 100.0)
}

/// One/five/fifteen-minute load averages via `getloadavg(3)`
/// on macOS and Linux; `0.0` where the OS has no native
/// reading.
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn load_average() -> [f64; 3] {
  let mut loads = [0.0f64; 3];

  // SAFETY: getloadavg writes up to 3 f64s into the buffer
  // and reads nothing; pointer and length are valid.
  unsafe {
    libc::getloadavg(loads.as_mut_ptr(), 3);
  }

  loads
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn load_average() -> [f64; 3] {
  [0.0; 3]
}

/// Current global CPU usage as a percentage in `0.0..=100.0`.
/// First call after process start returns `0.0`.
#[unsafe(no_mangle)]
pub extern "C-unwind" fn zo_sys_cpu_usage() -> f32 {
  imp::cpu_usage()
}

/// Number of logical CPUs. Backed by the std parallelism
/// query, so it needs no platform FFI and is correct on every
/// target.
#[unsafe(no_mangle)]
pub extern "C-unwind" fn zo_sys_cpu_count() -> i64 {
  std::thread::available_parallelism()
    .map(|count| count.get() as i64)
    .unwrap_or(1)
}

/// Total physical memory in bytes.
#[unsafe(no_mangle)]
pub extern "C-unwind" fn zo_sys_mem_total() -> i64 {
  imp::mem_total()
}

/// Used physical memory in bytes, excluding reclaimable cache
/// / buffers.
#[unsafe(no_mangle)]
pub extern "C-unwind" fn zo_sys_mem_used() -> i64 {
  imp::mem_used()
}

/// Available physical memory in bytes — what the OS would
/// hand a fresh allocator. Distinct from `total - used`
/// because `used` excludes reclaimable cache.
#[unsafe(no_mangle)]
pub extern "C-unwind" fn zo_sys_mem_available() -> i64 {
  imp::mem_available()
}

/// Total swap space in bytes. `0` when no swap is configured.
#[unsafe(no_mangle)]
pub extern "C-unwind" fn zo_sys_swap_total() -> i64 {
  imp::swap_total()
}

/// Used swap space in bytes.
#[unsafe(no_mangle)]
pub extern "C-unwind" fn zo_sys_swap_used() -> i64 {
  imp::swap_used()
}

/// System uptime in whole seconds — process-independent.
#[unsafe(no_mangle)]
pub extern "C-unwind" fn zo_sys_uptime_secs() -> i64 {
  imp::uptime_secs()
}

/// Number of running processes the OS reports.
#[unsafe(no_mangle)]
pub extern "C-unwind" fn zo_sys_proc_count() -> i64 {
  imp::proc_count()
}

/// One-minute load average. Windows: `0.0`.
#[unsafe(no_mangle)]
pub extern "C-unwind" fn zo_sys_load_avg_1m() -> f64 {
  load_average()[0]
}

/// Five-minute load average. Windows: `0.0`.
#[unsafe(no_mangle)]
pub extern "C-unwind" fn zo_sys_load_avg_5m() -> f64 {
  load_average()[1]
}

/// Fifteen-minute load average. Windows: `0.0`.
#[unsafe(no_mangle)]
pub extern "C-unwind" fn zo_sys_load_avg_15m() -> f64 {
  load_average()[2]
}

#[cfg(target_os = "macos")]
mod imp {
  use super::cpu_usage_delta;

  use std::ffi::{CStr, c_void};
  use std::sync::OnceLock;
  use std::{mem, ptr};

  /// libproc selector for "every process id".
  const PROC_ALL_PIDS: u32 = 1;

  // Mach + libproc symbols declared straight against
  // libSystem — libc's `mach_host_self` is deprecated and it
  // omits `proc_listpids` entirely.
  unsafe extern "C" {
    fn mach_host_self() -> libc::mach_port_t;

    fn proc_listpids(
      kind: u32,
      typeinfo: u32,
      buffer: *mut c_void,
      buffersize: libc::c_int,
    ) -> libc::c_int;
  }

  /// The host port, fetched once. A single send right leaks
  /// for the process lifetime — intentional, versus leaking
  /// one per memory / CPU query.
  fn host_port() -> libc::mach_port_t {
    static HOST_PORT: OnceLock<libc::mach_port_t> = OnceLock::new();

    // SAFETY: mach_host_self returns the caller's host port.
    *HOST_PORT.get_or_init(|| unsafe { mach_host_self() })
  }

  /// Read a named `u64` sysctl (e.g. `hw.memsize`), returning
  /// `0` if the query fails.
  fn sysctl_u64(name: &CStr) -> u64 {
    let mut value: u64 = 0;
    let mut size = mem::size_of::<u64>();

    // SAFETY: name is a valid C string; value/size describe a
    // u64-sized output buffer matching the requested length.
    unsafe {
      libc::sysctlbyname(
        name.as_ptr(),
        &mut value as *mut u64 as *mut c_void,
        &mut size,
        ptr::null_mut(),
        0,
      );
    }

    value
  }

  /// Mach VM page statistics plus the page size in bytes.
  /// Fields are zero on failure; callers clamp.
  fn vm_stats() -> (u64, libc::vm_statistics64) {
    // SAFETY: _SC_PAGESIZE is a valid sysconf key.
    let page = unsafe { libc::sysconf(libc::_SC_PAGESIZE) }.max(1) as u64;

    let mut info: libc::vm_statistics64 = unsafe { mem::zeroed() };
    let mut count = (mem::size_of::<libc::vm_statistics64>()
      / mem::size_of::<libc::integer_t>()) as u32;

    // SAFETY: info is sized to `count` integer_t slots; the
    // host port is valid for the call.
    unsafe {
      libc::host_statistics64(
        host_port(),
        libc::HOST_VM_INFO64,
        &mut info as *mut _ as *mut libc::integer_t,
        &mut count,
      );
    }

    (page, info)
  }

  /// Swap usage from `vm.swapusage`, in bytes. Zeroed on
  /// failure.
  fn swap_usage() -> libc::xsw_usage {
    let mut usage: libc::xsw_usage = unsafe { mem::zeroed() };
    let mut size = mem::size_of::<libc::xsw_usage>();

    // SAFETY: usage/size describe a buffer matching the
    // struct the kernel writes for this key.
    unsafe {
      libc::sysctlbyname(
        c"vm.swapusage".as_ptr(),
        &mut usage as *mut _ as *mut c_void,
        &mut size,
        ptr::null_mut(),
        0,
      );
    }

    usage
  }

  pub fn mem_total() -> i64 {
    sysctl_u64(c"hw.memsize").min(i64::MAX as u64) as i64
  }

  pub fn mem_used() -> i64 {
    let (page, vm) = vm_stats();
    let used = (vm.active_count as u64
      + vm.wire_count as u64
      + vm.compressor_page_count as u64)
      * page;

    used.min(i64::MAX as u64) as i64
  }

  pub fn mem_available() -> i64 {
    let (page, vm) = vm_stats();
    let avail = (vm.free_count as u64 + vm.inactive_count as u64) * page;

    avail.min(i64::MAX as u64) as i64
  }

  pub fn swap_total() -> i64 {
    swap_usage().xsu_total.min(i64::MAX as u64) as i64
  }

  pub fn swap_used() -> i64 {
    swap_usage().xsu_used.min(i64::MAX as u64) as i64
  }

  pub fn uptime_secs() -> i64 {
    let mut mib = [libc::CTL_KERN, libc::KERN_BOOTTIME];
    let mut boot: libc::timeval = unsafe { mem::zeroed() };
    let mut size = mem::size_of::<libc::timeval>();

    // SAFETY: mib holds `len` valid name elements; boot/size
    // describe a timeval-sized output buffer.
    unsafe {
      libc::sysctl(
        mib.as_mut_ptr(),
        mib.len() as u32,
        &mut boot as *mut _ as *mut c_void,
        &mut size,
        ptr::null_mut(),
        0,
      );
    }

    // SAFETY: a null buffer asks `time` only for its return.
    let now = unsafe { libc::time(ptr::null_mut()) };

    (now as i64 - boot.tv_sec as i64).max(0)
  }

  pub fn proc_count() -> i64 {
    // SAFETY: a null buffer makes proc_listpids return the
    // byte length it would fill; nothing is written.
    let bytes = unsafe { proc_listpids(PROC_ALL_PIDS, 0, ptr::null_mut(), 0) };

    if bytes <= 0 {
      return 1;
    }

    (bytes as usize / mem::size_of::<libc::c_int>()).max(1) as i64
  }

  pub fn cpu_usage() -> f32 {
    let mut info: libc::host_cpu_load_info = unsafe { mem::zeroed() };
    let mut count = (mem::size_of::<libc::host_cpu_load_info>()
      / mem::size_of::<libc::integer_t>()) as u32;

    // SAFETY: info is sized to `count` integer_t slots; the
    // host port is valid for the call.
    unsafe {
      libc::host_statistics(
        host_port(),
        libc::HOST_CPU_LOAD_INFO,
        &mut info as *mut _ as *mut libc::integer_t,
        &mut count,
      );
    }

    let ticks = info.cpu_ticks;
    let user = ticks[libc::CPU_STATE_USER as usize] as u64;
    let system = ticks[libc::CPU_STATE_SYSTEM as usize] as u64;
    let idle = ticks[libc::CPU_STATE_IDLE as usize] as u64;
    let nice = ticks[libc::CPU_STATE_NICE as usize] as u64;
    let busy = user + system + nice;

    cpu_usage_delta(busy, busy + idle)
  }
}

#[cfg(target_os = "linux")]
mod imp {
  use super::cpu_usage_delta;

  use std::mem;

  /// One `sysinfo(2)` snapshot. Zeroed on failure.
  fn snapshot() -> libc::sysinfo {
    let mut info: libc::sysinfo = unsafe { mem::zeroed() };

    // SAFETY: info points to a sysinfo-sized buffer the
    // kernel fills in full.
    unsafe {
      libc::sysinfo(&mut info);
    }

    info
  }

  /// The struct's byte multiplier for its memory fields,
  /// forced to at least 1.
  fn unit(info: &libc::sysinfo) -> u64 {
    (info.mem_unit as u64).max(1)
  }

  pub fn mem_total() -> i64 {
    let info = snapshot();
    (info.totalram * unit(&info)).min(i64::MAX as u64) as i64
  }

  pub fn mem_used() -> i64 {
    let info = snapshot();
    let used = info
      .totalram
      .saturating_sub(info.freeram)
      .saturating_sub(info.bufferram)
      * unit(&info);

    used.min(i64::MAX as u64) as i64
  }

  pub fn mem_available() -> i64 {
    let info = snapshot();
    ((info.freeram + info.bufferram) * unit(&info)).min(i64::MAX as u64) as i64
  }

  pub fn swap_total() -> i64 {
    let info = snapshot();
    (info.totalswap * unit(&info)).min(i64::MAX as u64) as i64
  }

  pub fn swap_used() -> i64 {
    let info = snapshot();
    (info.totalswap.saturating_sub(info.freeswap) * unit(&info))
      .min(i64::MAX as u64) as i64
  }

  pub fn uptime_secs() -> i64 {
    snapshot().uptime.max(0)
  }

  pub fn proc_count() -> i64 {
    (snapshot().procs as i64).max(1)
  }

  pub fn cpu_usage() -> f32 {
    // `/proc/stat`'s first line aggregates jiffies across all
    // CPUs: user nice system idle iowait irq softirq steal.
    let stat = match std::fs::read_to_string("/proc/stat") {
      Ok(stat) => stat,
      Err(_) => return 0.0,
    };

    let mut fields = stat.lines().next().unwrap_or("").split_whitespace();

    if fields.next() != Some("cpu") {
      return 0.0;
    }

    let counters: Vec<u64> = fields.filter_map(|f| f.parse().ok()).collect();

    if counters.len() < 4 {
      return 0.0;
    }

    let total: u64 = counters.iter().sum();
    let idle = counters[3] + counters.get(4).copied().unwrap_or(0);
    let busy = total.saturating_sub(idle);

    cpu_usage_delta(busy, total)
  }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
mod imp {
  // No native syscall path on this target (e.g. Windows,
  // which has no zo codegen backend yet). Memory / uptime /
  // process counters report `0`; `cpu_count` and the load
  // averages keep their cross-platform paths in the callers.

  pub fn cpu_usage() -> f32 {
    0.0
  }

  pub fn mem_total() -> i64 {
    0
  }

  pub fn mem_used() -> i64 {
    0
  }

  pub fn mem_available() -> i64 {
    0
  }

  pub fn swap_total() -> i64 {
    0
  }

  pub fn swap_used() -> i64 {
    0
  }

  pub fn uptime_secs() -> i64 {
    0
  }

  pub fn proc_count() -> i64 {
    0
  }
}
