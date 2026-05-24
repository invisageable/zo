//! OS-multiplexer-backed I/O readiness for green tasks.
//!
//! One [`Selector`] per scheduler OS thread (slated to
//! live inside `SchedulerState`). The Selector wraps
//! `kqueue` on macOS and `epoll` on Linux behind one
//! cfg-gated façade.
//!
//! Usage shape:
//!
//! - A suspending FFI (e.g. a future `_zo_net_read`)
//!   attempts a non-blocking syscall. On `EAGAIN`, it
//!   parks the current task by calling
//!   [`Selector::register_read`] with the fd and the
//!   task pointer, marks the task `Blocked`, and yields
//!   to the scheduler.
//! - The scheduler's run loop calls [`Selector::poll`]:
//!   with `0` for an opportunistic non-blocking drain
//!   after each task resume, with `-1` for the idle
//!   branch when the run queue is empty (sleeps inside
//!   the kernel until an fd fires).
//! - Returned task pointers are re-queued by the
//!   scheduler. The retry-loop in the FFI re-runs the
//!   syscall — succeeds with data, or sees `EAGAIN`
//!   again and re-registers (spurious-wake-safe).
//!
//! Design decisions locked in
//! `crates/compiler/zo-notes/personal/architecture/
//! PLAN_NET_NONBLOCKING_SOCKETS.md`:
//!
//! - Per-scheduler, never global — dodges the cross-
//!   thread green-task wake limitation documented in
//!   `channel.rs`.
//! - One-shot registration (`EV_ONESHOT` /
//!   `EPOLLONESHOT`) — exactly one wake per
//!   suspension; re-register on re-suspend.
//! - Level-triggered (no `EV_CLEAR`, no `EPOLLET`) —
//!   safe under partial reads / accept storms for the
//!   MVP; the FFI doesn't yet guarantee drain-to-EAGAIN
//!   loops at every site.
//! - `FxHashMap<RawFd, *mut ZoTask>` for the waiter
//!   table — handles fd reuse cleanly (`remove` erases
//!   stale entries), no resize allocation on hot path.

use std::os::raw::c_int;
use std::os::unix::io::RawFd;

use rustc_hash::FxHashMap as HashMap;

use crate::task::ZoTask;

/// Maximum events drained per [`Selector::poll`] call.
///
/// @note — 64 is the conventional size used by mio /
/// Go's netpoll. Large enough to amortize one syscall
/// across a burst of ready fds, small enough to keep
/// the stack-allocated event buffer cheap.
const POLL_BATCH: usize = 64;

/// OS-multiplexer-backed readiness queue. One instance
/// per scheduler OS thread; never crosses thread
/// boundaries (the raw task pointer field intentionally
/// makes the struct `!Send + !Sync`).
pub struct Selector {
  /// Multiplexer fd — `kqueue` handle on macOS, `epoll`
  /// instance on Linux. Closed in [`Drop`].
  mux_fd: RawFd,
  /// Tasks parked on an fd's readiness. Removed on
  /// wake; one-shot registration keeps this consistent
  /// across spurious events.
  ///
  /// @note — only one task per fd is supported in the
  /// MVP. Registering a second task on the same fd
  /// overwrites the first; the suspending-FFI pattern
  /// never triggers this because a task only parks on
  /// fds it owns.
  waiters: HashMap<RawFd, *mut ZoTask>,
}

impl Selector {
  /// Open a fresh multiplexer fd.
  ///
  /// @note — panics on failure. Scheduler construction
  /// can't proceed without a working multiplexer; a
  /// loud abort beats a silent runtime fallback that
  /// would re-introduce the blocking-syscall hazard.
  pub fn new() -> Self {
    let mux_fd = create_mux();

    if mux_fd < 0 {
      panic!(
        "zo-runtime: failed to create OS multiplexer fd \
         (errno={})",
        last_errno(),
      );
    }

    Self {
      mux_fd,
      waiters: HashMap::default(),
    }
  }

  /// Register interest in read readiness for `fd`. The
  /// associated `task` pointer is returned by [`poll`]
  /// once the OS reports `fd` as readable. One-shot —
  /// the kernel deregisters automatically on wake.
  pub fn register_read(&mut self, fd: RawFd, task: *mut ZoTask) {
    self.waiters.insert(fd, task);
    register(self.mux_fd, fd, Interest::Read);
  }

  /// Register interest in write readiness for `fd`.
  /// One-shot semantics, same as [`register_read`].
  pub fn register_write(&mut self, fd: RawFd, task: *mut ZoTask) {
    self.waiters.insert(fd, task);
    register(self.mux_fd, fd, Interest::Write);
  }

  /// Drain ready events.
  ///
  /// - `timeout_ms == -1` — block until at least one
  ///   fd fires (idle-poll mode).
  /// - `timeout_ms ==  0` — return immediately
  ///   (non-blocking opportunistic tick).
  /// - `timeout_ms  >  0` — block up to that many
  ///   milliseconds.
  ///
  /// Appends each ready task pointer onto `out`, in the
  /// order the kernel reported them. Ready tasks are
  /// removed from the waiter table. Caller supplies the
  /// buffer — the scheduler reuses one across quanta
  /// so a fresh allocation never lands on the run-loop
  /// hot path.
  pub fn poll(&mut self, timeout_ms: c_int, out: &mut Vec<*mut ZoTask>) {
    let waiters = &mut self.waiters;

    poll_ready(self.mux_fd, timeout_ms, |fd| {
      if let Some(task) = waiters.remove(&fd) {
        out.push(task);
      }
    });
  }

  /// Whether any tasks are parked on this selector.
  ///
  /// The scheduler queries this to decide between
  /// returning (no waiters, nothing left to wake) and
  /// blocking inside `poll(-1)` (waiters exist, kernel
  /// will wake us when any fires).
  pub fn has_waiters(&self) -> bool {
    !self.waiters.is_empty()
  }
}

impl Default for Selector {
  fn default() -> Self {
    Self::new()
  }
}

impl Drop for Selector {
  fn drop(&mut self) {
    if self.mux_fd >= 0 {
      // SAFETY: mux_fd was returned by kqueue() /
      // epoll_create1() in `new` and not closed since.
      unsafe {
        libc::close(self.mux_fd);
      }
    }
  }
}

#[derive(Clone, Copy)]
enum Interest {
  Read,
  Write,
}

// ===== macOS — kqueue backend =====

#[cfg(target_os = "macos")]
fn create_mux() -> RawFd {
  // SAFETY: kqueue() takes no args and returns either
  // a new fd or -1; both are safe to inspect.
  unsafe { libc::kqueue() }
}

#[cfg(target_os = "macos")]
fn register(mux_fd: RawFd, fd: RawFd, interest: Interest) {
  let filter = match interest {
    Interest::Read => libc::EVFILT_READ,
    Interest::Write => libc::EVFILT_WRITE,
  };

  // EV_ADD installs the registration; EV_ONESHOT makes
  // the kernel auto-deregister on the first delivery.
  let change = libc::kevent {
    ident: fd as libc::uintptr_t,
    filter,
    flags: libc::EV_ADD | libc::EV_ONESHOT,
    fflags: 0,
    data: 0,
    udata: std::ptr::null_mut(),
  };

  // SAFETY: change is a single, valid kevent on the
  // stack; eventlist is null with zero capacity so
  // kevent only processes the changelist.
  unsafe {
    libc::kevent(
      mux_fd,
      &change,
      1,
      std::ptr::null_mut(),
      0,
      std::ptr::null(),
    );
  }
}

#[cfg(target_os = "macos")]
fn poll_ready<F: FnMut(RawFd)>(
  mux_fd: RawFd,
  timeout_ms: c_int,
  mut on_ready: F,
) {
  // SAFETY: kevent is a plain POD struct; zeroing is
  // valid and matches the kernel's expectation for an
  // uninitialized eventlist buffer.
  let mut events: [libc::kevent; POLL_BATCH] = unsafe { std::mem::zeroed() };

  let timespec = if timeout_ms >= 0 {
    Some(libc::timespec {
      tv_sec: (timeout_ms / 1000) as libc::time_t,
      tv_nsec: ((timeout_ms % 1000) * 1_000_000) as _,
    })
  } else {
    None
  };
  let ts_ptr = timespec
    .as_ref()
    .map(|t| t as *const libc::timespec)
    .unwrap_or(std::ptr::null());

  // SAFETY: events is a stack-allocated array of
  // POLL_BATCH entries; capacity passed matches.
  let n = unsafe {
    libc::kevent(
      mux_fd,
      std::ptr::null(),
      0,
      events.as_mut_ptr(),
      POLL_BATCH as c_int,
      ts_ptr,
    )
  };

  if n <= 0 {
    return;
  }

  for ev in &events[..n as usize] {
    on_ready(ev.ident as RawFd);
  }
}

// ===== Linux — epoll backend =====

#[cfg(target_os = "linux")]
fn create_mux() -> RawFd {
  // SAFETY: epoll_create1 takes a flags int and returns
  // an fd or -1; both are safe to inspect.
  unsafe { libc::epoll_create1(libc::EPOLL_CLOEXEC) }
}

#[cfg(target_os = "linux")]
fn register(mux_fd: RawFd, fd: RawFd, interest: Interest) {
  let interest_mask = match interest {
    Interest::Read => libc::EPOLLIN,
    Interest::Write => libc::EPOLLOUT,
  };
  let mask = (interest_mask | libc::EPOLLONESHOT) as u32;

  let mut event = libc::epoll_event {
    events: mask,
    u64: fd as u64,
  };

  // One-shot fires once then disables the fd's
  // registration without removing it from the set. To
  // re-arm we use EPOLL_CTL_MOD; for first-time adds
  // that returns ENOENT, so we fall back to ADD.
  //
  // SAFETY: `event` is a valid epoll_event on the
  // stack; the kernel reads it during the syscall.
  let mod_rc =
    unsafe { libc::epoll_ctl(mux_fd, libc::EPOLL_CTL_MOD, fd, &mut event) };
  if mod_rc < 0 {
    // SAFETY: same as above.
    unsafe {
      libc::epoll_ctl(mux_fd, libc::EPOLL_CTL_ADD, fd, &mut event);
    }
  }
}

#[cfg(target_os = "linux")]
fn poll_ready<F: FnMut(RawFd)>(
  mux_fd: RawFd,
  timeout_ms: c_int,
  mut on_ready: F,
) {
  // SAFETY: epoll_event is a POD with a union payload;
  // zeroing is the conventional way to prep the buffer.
  let mut events: [libc::epoll_event; POLL_BATCH] =
    unsafe { std::mem::zeroed() };

  // SAFETY: events is a stack-allocated array of
  // POLL_BATCH entries; capacity passed matches.
  let n = unsafe {
    libc::epoll_wait(
      mux_fd,
      events.as_mut_ptr(),
      POLL_BATCH as c_int,
      timeout_ms,
    )
  };

  if n <= 0 {
    return;
  }

  for ev in &events[..n as usize] {
    on_ready(ev.u64 as RawFd);
  }
}

// ===== Suspending-FFI helpers =====

/// Park the currently-running green task on read
/// readiness for `fd`, then yield. When the kernel
/// reports `fd` as readable, the scheduler's idle-poll
/// re-queues the task.
///
/// Outside a task context (e.g. main-thread code that
/// hasn't entered a nursery), falls back to a blocking
/// `libc::poll` on the fd — single syscall, behaves
/// like the pre-Selector blocking read.
///
/// Callers loop: attempt the syscall, on `EAGAIN` /
/// `EWOULDBLOCK` call this helper, retry on resume.
/// The retry handles spurious wakes by re-encountering
/// `EAGAIN` and parking again.
pub fn park_current_on_read(fd: RawFd) {
  park_current(fd, Interest::Read);
}

/// Write-readiness counterpart to
/// [`park_current_on_read`]. Same semantics, but the
/// fd is registered for `EVFILT_WRITE` / `EPOLLOUT`.
pub fn park_current_on_write(fd: RawFd) {
  park_current(fd, Interest::Write);
}

/// EAGAIN classifier shared by every suspending FFI on
/// the read path (`_zo_net_read`, `_zo_net_tcp_accept`,
/// `_zo_io_read`, the buffered `_zo_io_readln` refill).
///
/// Returns `Some(-errno)` for a fatal errno that the
/// caller should propagate. Returns `None` after a
/// successful park — the caller loops and retries the
/// syscall. Wakes spuriously and re-encountering EAGAIN
/// is safe: the caller just parks again.
pub(crate) fn park_read_or_classify(fd: RawFd, errno: c_int) -> Option<i64> {
  if errno != libc::EAGAIN && errno != libc::EWOULDBLOCK {
    return Some(-(errno as i64));
  }

  park_current(fd, Interest::Read);
  None
}

/// Write-path counterpart to [`park_read_or_classify`].
pub(crate) fn park_write_or_classify(fd: RawFd, errno: c_int) -> Option<i64> {
  if errno != libc::EAGAIN && errno != libc::EWOULDBLOCK {
    return Some(-(errno as i64));
  }

  park_current(fd, Interest::Write);
  None
}

fn park_current(fd: RawFd, interest: Interest) {
  use crate::task::TaskState;

  let parked = crate::scheduler::with(|s| {
    let Some(task) = s.current() else {
      return false;
    };
    s.with_selector_mut(|sel| match interest {
      Interest::Read => sel.register_read(fd, task),
      Interest::Write => sel.register_write(fd, task),
    });
    // SAFETY: `task` is the running task on this OS
    // thread; the cooperative scheduler guarantees
    // exclusive access between yield boundaries.
    unsafe {
      (*task).state = TaskState::Blocked;
    }
    true
  });

  if parked {
    // SAFETY: we just confirmed via `s.current()` that
    // we are inside a green-task body.
    unsafe { crate::scheduler::yield_now() };
  } else {
    // Outside a task: block on `libc::poll` for the
    // requested readiness. Same effective cost as the
    // pre-Selector blocking read.
    let events = match interest {
      Interest::Read => libc::POLLIN,
      Interest::Write => libc::POLLOUT,
    };
    let mut pfd = libc::pollfd {
      fd,
      events,
      revents: 0,
    };
    // SAFETY: pfd is a valid pollfd on the stack;
    // count matches the array length (1).
    unsafe {
      libc::poll(&mut pfd, 1, -1);
    }
  }
}

// ===== Errno helper =====

pub(crate) fn last_errno() -> c_int {
  // SAFETY: __error/__errno_location returns a thread-
  // local pointer that's always valid for the calling
  // thread's lifetime.
  unsafe { *errno_location() }
}

#[cfg(target_os = "macos")]
unsafe fn errno_location() -> *mut c_int {
  unsafe extern "C" {
    fn __error() -> *mut c_int;
  }
  // SAFETY: see above.
  unsafe { __error() }
}

#[cfg(target_os = "linux")]
unsafe fn errno_location() -> *mut c_int {
  unsafe extern "C" {
    fn __errno_location() -> *mut c_int;
  }
  // SAFETY: see above.
  unsafe { __errno_location() }
}

// ===== TCP FFI surface =====
//
// Six C-ABI entry points. Sockets are non-blocking;
// `accept` / `connect` / `read` / `write` route EAGAIN
// through the suspending park helpers. Return convention:
// non-negative on success, `-errno` on failure.

use libc::c_char;

/// Bind a listening TCP socket. `host` is a NUL-
/// terminated IPv4 or IPv6 literal (no port suffix);
/// `port` is the TCP port (`0` for OS-assigned).
/// Returns the listener fd or `-errno`.
///
/// # Safety
///
/// `host` must point at a NUL-terminated UTF-8 string
/// or be null.
#[unsafe(export_name = "zo_net_tcp_listen")]
pub unsafe extern "C-unwind" fn _zo_net_tcp_listen(
  host: *const c_char,
  port: i64,
) -> i64 {
  let Some(sa) = (unsafe { parse_host_port(host, port) }) else {
    return -(libc::EINVAL as i64);
  };

  let family = match sa {
    std::net::SocketAddr::V4(_) => libc::AF_INET,
    std::net::SocketAddr::V6(_) => libc::AF_INET6,
  };

  let fd = create_stream_socket(family);
  if fd < 0 {
    return -(last_errno() as i64);
  }

  // SO_REUSEADDR — restart-friendly. Default for any
  // server you'd actually deploy.
  let yes: i32 = 1;
  // SAFETY: `yes` lives on the stack for the call.
  unsafe {
    libc::setsockopt(
      fd,
      libc::SOL_SOCKET,
      libc::SO_REUSEADDR,
      &yes as *const i32 as *const _,
      std::mem::size_of::<i32>() as libc::socklen_t,
    );
  }

  let (storage, len) = sockaddr_storage(&sa);
  // SAFETY: storage is a valid sockaddr_storage of `len`
  // bytes; family matches the storage's `_in` / `_in6`
  // initialization in `sockaddr_storage`.
  let rc = unsafe {
    libc::bind(
      fd,
      &storage as *const libc::sockaddr_storage as *const _,
      len,
    )
  };
  if rc < 0 {
    let err = last_errno();
    // SAFETY: fd just opened above; closing is safe.
    unsafe {
      libc::close(fd);
    }
    return -(err as i64);
  }

  // SAFETY: fd is a valid socket.
  let rc = unsafe { libc::listen(fd, 128) };
  if rc < 0 {
    let err = last_errno();
    unsafe {
      libc::close(fd);
    }
    return -(err as i64);
  }

  fd as i64
}

/// Accept the next pending connection on `fd`. Suspends
/// the calling task on listener readiness until a
/// connection arrives. Returns the accepted fd or
/// `-errno`.
///
/// # Safety
///
/// `fd` must be a valid listening socket previously
/// returned by [`_zo_net_tcp_listen`].
#[unsafe(export_name = "zo_net_tcp_accept")]
pub unsafe extern "C-unwind" fn _zo_net_tcp_accept(fd: i64) -> i64 {
  let fd = fd as RawFd;

  loop {
    let new_fd = do_accept(fd);

    if new_fd >= 0 {
      return new_fd as i64;
    }

    // Accept readiness arrives as a read event on the
    // listener fd.
    if let Some(err) = park_read_or_classify(fd, last_errno()) {
      return err;
    }
  }
}

/// Open a TCP connection to `addr`. `addr` matches the
/// listen format (`"host:port"`). Suspends the calling
/// task on connect completion (write-readiness on the
/// connecting socket). Returns the connected fd or
/// `-errno`.
///
/// # Safety
///
/// `host` must point at a NUL-terminated UTF-8 IP
/// literal or be null.
#[unsafe(export_name = "zo_net_tcp_connect")]
pub unsafe extern "C-unwind" fn _zo_net_tcp_connect(
  host: *const c_char,
  port: i64,
) -> i64 {
  let Some(sa) = (unsafe { parse_host_port(host, port) }) else {
    return -(libc::EINVAL as i64);
  };

  let family = match sa {
    std::net::SocketAddr::V4(_) => libc::AF_INET,
    std::net::SocketAddr::V6(_) => libc::AF_INET6,
  };

  let fd = create_stream_socket(family);
  if fd < 0 {
    return -(last_errno() as i64);
  }

  let (storage, len) = sockaddr_storage(&sa);

  // SAFETY: storage is a valid sockaddr_storage.
  let rc = unsafe {
    libc::connect(
      fd,
      &storage as *const libc::sockaddr_storage as *const _,
      len,
    )
  };

  if rc == 0 {
    // Loopback connects often complete synchronously.
    return fd as i64;
  }

  let err = last_errno();
  if err != libc::EINPROGRESS && err != libc::EWOULDBLOCK {
    unsafe {
      libc::close(fd);
    }
    return -(err as i64);
  }

  // Connect is in progress — wait for write-readiness,
  // which signals completion (success or async fail).
  park_current_on_write(fd);

  // Check SO_ERROR — an async connect failure surfaces
  // there, not as the next syscall's errno.
  let mut soerr: i32 = 0;
  let mut soerr_len = std::mem::size_of::<i32>() as libc::socklen_t;

  // SAFETY: soerr and soerr_len live on the stack.
  let rc = unsafe {
    libc::getsockopt(
      fd,
      libc::SOL_SOCKET,
      libc::SO_ERROR,
      &mut soerr as *mut i32 as *mut _,
      &mut soerr_len,
    )
  };

  if rc < 0 {
    let err = last_errno();
    unsafe {
      libc::close(fd);
    }
    return -(err as i64);
  }
  if soerr != 0 {
    unsafe {
      libc::close(fd);
    }
    return -(soerr as i64);
  }

  fd as i64
}

/// Read up to `n` bytes from `fd` into `buf`. Suspends
/// on read-readiness if the socket has no data. Returns
/// bytes read (0 = EOF) or `-errno`.
///
/// # Safety
///
/// `buf` must point at `n` writable bytes that stay
/// valid for the call (including across yield points).
/// `fd` must be a valid socket.
#[unsafe(export_name = "zo_net_read")]
pub unsafe extern "C-unwind" fn _zo_net_read(
  fd: i64,
  buf: *mut u8,
  n: usize,
) -> isize {
  let fd = fd as RawFd;

  loop {
    // SAFETY: caller contract — buf/n form a valid
    // writable region.
    let r = unsafe { libc::read(fd, buf as *mut _, n) };

    if r >= 0 {
      return r;
    }

    if let Some(err) = park_read_or_classify(fd, last_errno()) {
      return err as isize;
    }
  }
}

/// Write up to `n` bytes from `buf` to `fd`. Suspends
/// on write-readiness if the socket's send buffer is
/// full. Returns bytes written or `-errno`.
///
/// @note — partial writes are possible; callers loop
/// at the source level if they need full coverage. The
/// `core/net.zo` `tcp_write` exposes the raw result.
///
/// # Safety
///
/// `buf` must point at `n` readable bytes valid for
/// the call (including across yield points). `fd` must
/// be a valid socket.
#[unsafe(export_name = "zo_net_write")]
pub unsafe extern "C-unwind" fn _zo_net_write(
  fd: i64,
  buf: *const u8,
  n: usize,
) -> isize {
  let fd = fd as RawFd;

  loop {
    // SAFETY: caller contract.
    let r = unsafe { libc::write(fd, buf as *const _, n) };

    if r >= 0 {
      return r;
    }

    if let Some(err) = park_write_or_classify(fd, last_errno()) {
      return err as isize;
    }
  }
}

/// Look up the local port `fd` is bound to. Servers
/// that bind on port `0` (OS-assigned) use this to tell
/// clients where to connect. Returns the port or
/// `-errno`. Supports `AF_INET` and `AF_INET6`.
///
/// # Safety
///
/// `fd` must be a valid bound socket.
#[unsafe(export_name = "zo_net_tcp_local_port")]
pub unsafe extern "C-unwind" fn _zo_net_tcp_local_port(fd: i64) -> i64 {
  let fd = fd as RawFd;
  let mut storage: libc::sockaddr_storage = unsafe { std::mem::zeroed() };
  let mut len =
    std::mem::size_of::<libc::sockaddr_storage>() as libc::socklen_t;

  // SAFETY: storage + len live on this stack frame.
  let rc = unsafe {
    libc::getsockname(
      fd,
      &mut storage as *mut libc::sockaddr_storage as *mut _,
      &mut len,
    )
  };

  if rc < 0 {
    return -(last_errno() as i64);
  }

  let family = storage.ss_family as c_int;
  let port = if family == libc::AF_INET {
    // SAFETY: family verified; storage outlives borrow.
    let sin = unsafe {
      &*(&storage as *const libc::sockaddr_storage as *const libc::sockaddr_in)
    };
    u16::from_be(sin.sin_port)
  } else if family == libc::AF_INET6 {
    let sin6 = unsafe {
      &*(&storage as *const libc::sockaddr_storage as *const libc::sockaddr_in6)
    };
    u16::from_be(sin6.sin6_port)
  } else {
    return -(libc::EAFNOSUPPORT as i64);
  };

  port as i64
}

/// Close `fd`. Returns 0 on success, `-errno` on
/// failure.
///
/// # Safety
///
/// `fd` must be a valid socket; after this call, no
/// further FFI may reference `fd`.
#[unsafe(export_name = "zo_net_close")]
pub unsafe extern "C-unwind" fn _zo_net_close(fd: i64) -> i64 {
  // SAFETY: caller contract — fd is a valid open fd.
  let rc = unsafe { libc::close(fd as RawFd) };

  if rc < 0 { -(last_errno() as i64) } else { 0 }
}

// ===== Str-shaped convenience FFIs =====
//
// Thin shims that bridge zo's `str` / `CBytes` types to
// the raw read/write loops above. Keep `core/net.zo`'s
// `TcpStream::read` / `write` Rust-like (`str` in, `str`
// out) without exposing buffer pointers at the source
// surface.

/// Write the byte payload of zo str `s` to `fd`, looping
/// over partial writes until all bytes are sent or an
/// error occurs. Returns total bytes written (`==`
/// `s.len()` on success) or `-errno` on failure.
///
/// # Safety
///
/// `s` must point at a valid zo str header
/// (`[len:u64][bytes][NUL]`) or be null.
#[unsafe(export_name = "zo_net_write_str")]
pub unsafe extern "C-unwind" fn _zo_net_write_str(
  fd: i64,
  s: *const u8,
) -> i64 {
  if s.is_null() {
    return -(libc::EINVAL as i64);
  }

  // SAFETY: caller contract — s is a valid str header.
  let bytes = unsafe { crate::str::str_bytes(s) };
  let total = bytes.len();
  let payload = bytes.as_ptr();

  let mut written: usize = 0;

  while written < total {
    // SAFETY: payload + written stays inside [payload,
    // payload + total) which is within the str header.
    let n = unsafe { _zo_net_write(fd, payload.add(written), total - written) };

    if n < 0 {
      return n as i64;
    }

    written += n as usize;
  }

  total as i64
}

/// Maximum read size satisfied without a heap scratch.
/// Sized to match the typical socket MTU / page so the
/// common case never leaves the on-stack buffer. Each
/// green task owns its own stack (`TaskStack`), so this
/// is re-entrancy-safe across `yield_now`.
const NET_READ_STACK_BUF: usize = 4096;

/// Read up to `max_len` bytes from `fd` into a freshly
/// heap-allocated zo str. Returns `CBytes` whose `ptr`
/// points at the str's byte payload and whose `len`
/// carries the byte count.
///
/// Error encoding: `ptr == null && len < 0` ⇒ `-errno`.
/// EOF or `max_len == 0`: `ptr == &NUL && len == 0`
/// (a valid empty `CBytes` via `CBytes::empty()`).
///
/// The zo-side wrapper turns `CBytes` into a `str` via
/// `CBytes::to_str()` — keeps the heap-str ownership
/// model uniform with hash/regex returns.
///
/// Scratch lives on the calling green task's own
/// stack — survives yields, no heap until the final
/// str header.
///
/// # Safety
///
/// `fd` must be a valid socket.
#[unsafe(export_name = "zo_net_read_to_str")]
pub unsafe extern "C-unwind" fn _zo_net_read_to_str(
  fd: i64,
  max_len: i64,
) -> zo_c_abi::CBytes {
  use std::os::raw::c_char;

  if !(0..=64 * 1024 * 1024).contains(&max_len) {
    return zo_c_abi::CBytes {
      ptr: std::ptr::null(),
      len: -(libc::EINVAL as i64),
    };
  }

  let max_len = max_len as usize;

  if max_len == 0 {
    return zo_c_abi::CBytes::empty();
  }

  // Per-task stack scratch. Each green task has its own
  // 8 MiB stack; this buffer lives alongside the call's
  // other locals and survives `yield_now` because the
  // task's SP is preserved across context switches.
  // Reads larger than `NET_READ_STACK_BUF` are clamped
  // (matches POSIX `read` semantics — partial returns
  // are normal, callers loop).
  let mut stack_buf = [0u8; NET_READ_STACK_BUF];
  let read_len = max_len.min(NET_READ_STACK_BUF);

  // SAFETY: stack_buf is a live local; pointer/len match.
  let n = unsafe { _zo_net_read(fd, stack_buf.as_mut_ptr(), read_len) };

  if n < 0 {
    return zo_c_abi::CBytes {
      ptr: std::ptr::null(),
      len: n as i64,
    };
  }

  let n = n as usize;

  if n == 0 {
    return zo_c_abi::CBytes::empty();
  }

  // Heap-alloc a zo str of exactly `n` bytes; payload
  // sits at offset 8. This is the only allocation on
  // the read path.
  let header_ptr = crate::str::alloc_str(&stack_buf[..n]);

  // SAFETY: header_ptr was just allocated; +8 is the
  // payload start. The heap allocation outlives the
  // returned CBytes (zo currently leaks str heap memory).
  zo_c_abi::CBytes {
    ptr: unsafe { header_ptr.add(8) } as *const c_char,
    len: n as i64,
  }
}

// ===== Socket helpers =====

/// Create a non-blocking, close-on-exec stream socket
/// for `family` (`AF_INET` or `AF_INET6`). Returns the
/// fd or `-1` (errno set).
fn create_stream_socket(family: i32) -> RawFd {
  #[cfg(target_os = "linux")]
  {
    // SAFETY: socket() with documented args.
    unsafe {
      libc::socket(
        family,
        libc::SOCK_STREAM | libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC,
        0,
      )
    }
  }
  #[cfg(target_os = "macos")]
  {
    // SAFETY: socket() with documented args.
    let fd = unsafe { libc::socket(family, libc::SOCK_STREAM, 0) };

    if fd < 0 {
      return fd;
    }

    set_nonblocking(fd);
    set_cloexec(fd);

    fd
  }
}

/// `fcntl(fd, getter) | flag` followed by
/// `fcntl(fd, setter, …)`. Used to enable
/// `O_NONBLOCK` / `FD_CLOEXEC` on existing fds on
/// macOS — Linux opens sockets with the equivalent
/// `SOCK_NONBLOCK | SOCK_CLOEXEC` flags directly so
/// the after-the-fact `fcntl` path is unused there.
fn fcntl_or(fd: RawFd, getter: c_int, setter: c_int, flag: c_int) {
  // SAFETY: fcntl with documented cmds on a valid fd.
  unsafe {
    let flags = libc::fcntl(fd, getter);

    if flags >= 0 {
      libc::fcntl(fd, setter, flags | flag);
    }
  }
}

/// Mark `fd` as `O_NONBLOCK`. Shared by the socket FFI
/// and `io::ensure_stdin_nonblocking` so the suspending
/// pattern reads from one place. On Linux we open
/// sockets with `SOCK_NONBLOCK` directly, but stdin
/// still flows through this helper.
pub(crate) fn set_nonblocking(fd: RawFd) {
  fcntl_or(fd, libc::F_GETFL, libc::F_SETFL, libc::O_NONBLOCK);
}

#[cfg(target_os = "macos")]
fn set_cloexec(fd: RawFd) {
  fcntl_or(fd, libc::F_GETFD, libc::F_SETFD, libc::FD_CLOEXEC);
}

/// Accept one connection, returning a non-blocking,
/// close-on-exec fd or `-1` (errno set).
fn do_accept(fd: RawFd) -> RawFd {
  #[cfg(target_os = "linux")]
  {
    // SAFETY: accept4 with documented args.
    unsafe {
      libc::accept4(
        fd,
        std::ptr::null_mut(),
        std::ptr::null_mut(),
        libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC,
      )
    }
  }
  #[cfg(target_os = "macos")]
  {
    // SAFETY: accept with documented args.
    let new_fd =
      unsafe { libc::accept(fd, std::ptr::null_mut(), std::ptr::null_mut()) };

    if new_fd >= 0 {
      set_nonblocking(new_fd);
      set_cloexec(new_fd);
    }

    new_fd
  }
}

/// Parse `(host: CStr, port: i64)` into a `SocketAddr`.
/// `host` is a plain IPv4 / IPv6 literal (no port,
/// no brackets); `port` must fit in `u16`. Returns
/// `None` for null host, non-UTF-8, unparseable IP,
/// or out-of-range port.
unsafe fn parse_host_port(
  host: *const c_char,
  port: i64,
) -> Option<std::net::SocketAddr> {
  if host.is_null() || !(0..=u16::MAX as i64).contains(&port) {
    return None;
  }

  // SAFETY: caller contract — host is NUL-terminated.
  let cstr = unsafe { std::ffi::CStr::from_ptr(host) };
  let ip: std::net::IpAddr = cstr.to_str().ok()?.parse().ok()?;

  Some(std::net::SocketAddr::new(ip, port as u16))
}

/// Marshal a Rust `SocketAddr` into a kernel-friendly
/// `sockaddr_storage` + matching `socklen_t`.
///
/// @note — the macOS / FreeBSD `sin_len` byte is set
/// per BSD convention; Linux doesn't have it.
fn sockaddr_storage(
  addr: &std::net::SocketAddr,
) -> (libc::sockaddr_storage, libc::socklen_t) {
  // SAFETY: sockaddr_storage is a POD union large
  // enough for both _in and _in6; zero init is valid.
  let mut storage: libc::sockaddr_storage = unsafe { std::mem::zeroed() };

  let len = match addr {
    std::net::SocketAddr::V4(v4) => {
      // SAFETY: sockaddr_storage is sized for any
      // address family; aliasing as sockaddr_in is the
      // standard pattern.
      let sin = unsafe {
        &mut *(&mut storage as *mut libc::sockaddr_storage
          as *mut libc::sockaddr_in)
      };

      #[cfg(any(target_os = "macos", target_os = "freebsd"))]
      {
        sin.sin_len = std::mem::size_of::<libc::sockaddr_in>() as u8;
      }
      sin.sin_family = libc::AF_INET as _;
      sin.sin_port = v4.port().to_be();
      sin.sin_addr.s_addr = u32::from(*v4.ip()).to_be();

      std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t
    }
    std::net::SocketAddr::V6(v6) => {
      // SAFETY: same as above for sockaddr_in6.
      let sin6 = unsafe {
        &mut *(&mut storage as *mut libc::sockaddr_storage
          as *mut libc::sockaddr_in6)
      };

      #[cfg(any(target_os = "macos", target_os = "freebsd"))]
      {
        sin6.sin6_len = std::mem::size_of::<libc::sockaddr_in6>() as u8;
      }
      sin6.sin6_family = libc::AF_INET6 as _;
      sin6.sin6_port = v6.port().to_be();
      sin6.sin6_addr.s6_addr = v6.ip().octets();
      sin6.sin6_flowinfo = v6.flowinfo();
      sin6.sin6_scope_id = v6.scope_id();

      std::mem::size_of::<libc::sockaddr_in6>() as libc::socklen_t
    }
  };

  (storage, len)
}

// ===== Tests =====

#[cfg(test)]
mod tests {
  use super::*;

  /// Sentinel task pointer. The selector never
  /// dereferences task pointers — only stores and
  /// returns them — so any non-null bit pattern works.
  fn sentinel(tag: usize) -> *mut ZoTask {
    tag as *mut ZoTask
  }

  /// Pipe with both ends set non-blocking, mirroring
  /// the shape sockets will take once Step 4 lands.
  fn pipe_nb() -> (RawFd, RawFd) {
    let mut fds = [0i32; 2];

    let rc = unsafe { libc::pipe(fds.as_mut_ptr()) };
    assert_eq!(rc, 0, "pipe() failed");

    for &fd in &fds {
      unsafe {
        let flags = libc::fcntl(fd, libc::F_GETFL);
        libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
      }
    }

    (fds[0], fds[1])
  }

  fn close_pipe(fds: (RawFd, RawFd)) {
    unsafe {
      libc::close(fds.0);
      libc::close(fds.1);
    }
  }

  fn drain(sel: &mut Selector, timeout_ms: c_int) -> Vec<*mut ZoTask> {
    let mut out = Vec::new();
    sel.poll(timeout_ms, &mut out);
    out
  }

  #[test]
  fn poll_returns_no_tasks_when_idle() {
    let mut sel = Selector::new();

    assert!(!sel.has_waiters());
    assert!(drain(&mut sel, 0).is_empty());
  }

  #[test]
  fn poll_zero_timeout_is_non_blocking() {
    let mut sel = Selector::new();

    let start = std::time::Instant::now();
    let ready = drain(&mut sel, 0);
    let elapsed = start.elapsed();

    assert!(ready.is_empty());
    assert!(
      elapsed < std::time::Duration::from_millis(50),
      "poll(0) took too long: {:?}",
      elapsed,
    );
  }

  #[test]
  fn register_read_wakes_when_data_arrives() {
    let (read_fd, write_fd) = pipe_nb();
    let task = sentinel(0xCAFE);

    let mut sel = Selector::new();
    sel.register_read(read_fd, task);

    assert!(sel.has_waiters());

    // Empty pipe — nothing ready yet.
    assert!(drain(&mut sel, 0).is_empty());
    assert!(sel.has_waiters());

    // Inject one byte; the read end becomes readable.
    let byte = b"x";
    let n = unsafe { libc::write(write_fd, byte.as_ptr() as *const _, 1) };
    assert_eq!(n, 1);

    // Block up to 500 ms — should fire long before.
    let ready = drain(&mut sel, 500);
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0], task);
    // One-shot — waiter removed on wake.
    assert!(!sel.has_waiters());

    close_pipe((read_fd, write_fd));
  }

  #[test]
  fn multiple_fds_each_wake_their_own_task() {
    let pipe_a = pipe_nb();
    let pipe_b = pipe_nb();
    let task_a = sentinel(0xAAAA);
    let task_b = sentinel(0xBBBB);

    let mut sel = Selector::new();
    sel.register_read(pipe_a.0, task_a);
    sel.register_read(pipe_b.0, task_b);

    // Only pipe B receives data — only task B wakes.
    let byte = b"y";
    unsafe {
      libc::write(pipe_b.1, byte.as_ptr() as *const _, 1);
    }

    let ready = drain(&mut sel, 500);

    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0], task_b);
    // task_a still parked on pipe_a's read end.
    assert!(sel.has_waiters());

    close_pipe(pipe_a);
    close_pipe(pipe_b);
  }

  #[test]
  fn reregister_after_wake_succeeds() {
    let (read_fd, write_fd) = pipe_nb();
    let task = sentinel(0xDEAD);

    let mut sel = Selector::new();

    // First arm: register, deliver, wake.
    sel.register_read(read_fd, task);
    unsafe {
      libc::write(write_fd, b"a".as_ptr() as *const _, 1);
    }
    let ready = drain(&mut sel, 500);
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0], task);

    // Drain the byte so the pipe is empty again.
    let mut buf = [0u8; 1];
    let _ = unsafe { libc::read(read_fd, buf.as_mut_ptr() as *mut _, 1) };

    // Second arm — exercises the EPOLL_CTL_MOD re-arm
    // path on Linux and the redundant EV_ADD on macOS.
    sel.register_read(read_fd, task);
    unsafe {
      libc::write(write_fd, b"b".as_ptr() as *const _, 1);
    }
    let ready = drain(&mut sel, 500);
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0], task);

    close_pipe((read_fd, write_fd));
  }

  // ===== TCP roundtrip test =====
  //
  // Two green tasks share a TCP loopback connection: the
  // server echoes "world" back to the client's "hello".

  use crate::task::{_zo_task_await, _zo_task_spawn_2};

  /// Read the OS-assigned port of a freshly bound
  /// listener. Test-only helper around `getsockname`.
  fn local_port_v4(fd: RawFd) -> u16 {
    let mut sin: libc::sockaddr_in = unsafe { std::mem::zeroed() };
    let mut len = std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t;
    let rc = unsafe {
      libc::getsockname(
        fd,
        &mut sin as *mut libc::sockaddr_in as *mut _,
        &mut len,
      )
    };
    assert_eq!(rc, 0, "getsockname failed");
    u16::from_be(sin.sin_port)
  }

  extern "C-unwind" fn server_body(listener_fd: u64, _unused: u64) {
    let listener_fd = listener_fd as i64;

    let conn = unsafe { _zo_net_tcp_accept(listener_fd) };
    assert!(conn >= 0, "accept failed: {}", conn);

    // Drain "hello" — the client writes exactly 5 bytes
    // so a single read call is sufficient in practice.
    let mut buf = [0u8; 5];
    let n = unsafe { _zo_net_read(conn, buf.as_mut_ptr(), 5) };
    assert_eq!(n, 5, "server read returned {}", n);
    assert_eq!(&buf, b"hello");

    let n = unsafe { _zo_net_write(conn, b"world".as_ptr(), 5) };
    assert_eq!(n, 5, "server write returned {}", n);

    unsafe { _zo_net_close(conn) };
  }

  extern "C-unwind" fn client_body(port: u64, _unused: u64) {
    let host = std::ffi::CString::new("127.0.0.1").unwrap();

    let conn = unsafe { _zo_net_tcp_connect(host.as_ptr(), port as i64) };
    assert!(conn >= 0, "connect failed: {}", conn);

    let n = unsafe { _zo_net_write(conn, b"hello".as_ptr(), 5) };
    assert_eq!(n, 5, "client write returned {}", n);

    let mut buf = [0u8; 5];
    let n = unsafe { _zo_net_read(conn, buf.as_mut_ptr(), 5) };
    assert_eq!(n, 5, "client read returned {}", n);
    assert_eq!(&buf, b"world");

    unsafe { _zo_net_close(conn) };
  }

  #[test]
  fn tcp_loopback_roundtrip() {
    crate::scheduler::reset_for_test();

    let listen_host = std::ffi::CString::new("127.0.0.1").unwrap();
    let listener = unsafe { _zo_net_tcp_listen(listen_host.as_ptr(), 0) };
    assert!(listener >= 0, "listen failed: {}", listener);
    let listener_fd = listener as RawFd;
    let port = local_port_v4(listener_fd);

    let server = unsafe { _zo_task_spawn_2(server_body, listener as u64, 0) };
    let client = unsafe { _zo_task_spawn_2(client_body, port as u64, 0) };

    // SAFETY: handles are fresh from spawn_2; their
    // boxes outlive these awaits.
    unsafe {
      _zo_task_await(server);
      _zo_task_await(client);
    }

    unsafe {
      libc::close(listener_fd);
    }
  }
}
