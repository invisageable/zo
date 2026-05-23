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
  /// Returns the task pointers whose fds are now
  /// ready, in the order the kernel reported them.
  /// Ready tasks are removed from the waiter table.
  pub fn poll(&mut self, timeout_ms: c_int) -> Vec<*mut ZoTask> {
    let mut ready: Vec<*mut ZoTask> = Vec::new();

    poll_ready(self.mux_fd, timeout_ms, |fd| {
      if let Some(task) = self.waiters.remove(&fd) {
        ready.push(task);
      }
    });

    ready
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
  let mut events: [libc::kevent; POLL_BATCH] =
    unsafe { std::mem::zeroed() };

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
  let mod_rc = unsafe {
    libc::epoll_ctl(mux_fd, libc::EPOLL_CTL_MOD, fd, &mut event)
  };
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

// ===== Errno helper =====

fn last_errno() -> c_int {
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

  #[test]
  fn poll_returns_no_tasks_when_idle() {
    let mut sel = Selector::new();

    assert!(!sel.has_waiters());

    let ready = sel.poll(0);

    assert!(ready.is_empty());
  }

  #[test]
  fn poll_zero_timeout_is_non_blocking() {
    let mut sel = Selector::new();

    let start = std::time::Instant::now();
    let ready = sel.poll(0);
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
    let ready = sel.poll(0);
    assert!(ready.is_empty());
    assert!(sel.has_waiters());

    // Inject one byte; the read end becomes readable.
    let byte = b"x";
    let n =
      unsafe { libc::write(write_fd, byte.as_ptr() as *const _, 1) };
    assert_eq!(n, 1);

    // Block up to 500 ms — should fire long before.
    let ready = sel.poll(500);
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

    let ready = sel.poll(500);

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
    let ready = sel.poll(500);
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0], task);

    // Drain the byte so the pipe is empty again.
    let mut buf = [0u8; 1];
    let _ = unsafe {
      libc::read(read_fd, buf.as_mut_ptr() as *mut _, 1)
    };

    // Second arm — exercises the EPOLL_CTL_MOD re-arm
    // path on Linux and the redundant EV_ADD on macOS.
    sel.register_read(read_fd, task);
    unsafe {
      libc::write(write_fd, b"b".as_ptr() as *const _, 1);
    }
    let ready = sel.poll(500);
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0], task);

    close_pipe((read_fd, write_fd));
  }
}
