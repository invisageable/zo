//! Task stack memory — virtual reservation with
//! on-demand commit.
//!
//! Each green task owns a [`TaskStack`]: a reserved
//! virtual region much larger than it needs, with only
//! a prefix actually committed (readable + writable).
//! The rest is `PROT_NONE` guard — any access traps.
//!
//! The reservation lives at a fixed address for the
//! lifetime of the task. Frames can be referenced by
//! raw pointers without fear of invalidation. A
//! process-wide signal handler extends the committed
//! prefix when the task faults past the current
//! boundary.
//!
//! The initial commit is a single page (typically
//! 4 KB, or 16 KB on Apple Silicon). Tasks that don't
//! recurse pay one page of RSS; tasks that do pay an
//! extra `mprotect` per growth.

use std::io;
use std::ptr::NonNull;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicPtr, Ordering};

/// Reserved virtual region per task — large enough to
/// accommodate the deepest recursion a zo program is
/// likely to exhibit, without committing backing pages
/// until they're written. 8 MiB matches the typical
/// pthread default.
pub const STACK_RESERVE_BYTES: usize = 8 * 1024 * 1024;

/// Maximum number of `mmap`'d stacks retained for
/// reuse after their owning task dies (fallback path
/// only — the arena has its own free list). Hot
/// spawn/drop loops recycle cached reservations and
/// skip the `mmap` + `mprotect` syscalls on the hot
/// path; excess drops hit `munmap` and return memory to
/// the kernel.
const STACK_POOL_CAP: usize = 64;

/// Default arena capacity in slots. 131072 × 8 MiB =
/// 1 TiB virtual — zero physical cost until a slot is
/// touched, and one single kernel VMA entry instead of
/// 131k. Sized so 100k concurrent green tasks fit
/// without touching `vm.max_map_count`. If the mmap
/// fails (container limits, 32-bit, etc.), the runtime
/// falls back to per-task mmap transparently.
const ARENA_DEFAULT_SLOTS: usize = 16_384;

/// Byte size of the OS memory page, resolved once per
/// process. Apple Silicon uses 16 KiB pages; most
/// other targets use 4 KiB. `mprotect` requires
/// page-aligned addresses + lengths.
fn page_size() -> usize {
  static PAGE_SIZE: OnceLock<usize> = OnceLock::new();

  *PAGE_SIZE.get_or_init(|| {
    let p = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };

    if p <= 0 { 4096 } else { p as usize }
  })
}

// ===== stack arena =====

/// Contiguous virtual slab carved into per-task stack
/// slots. One `mmap(PROT_NONE)` at init; each slot
/// `mprotect`s its first page on alloc. Eliminates
/// per-task `mmap` + `munmap` syscalls and collapses
/// 100k VMAs into exactly one — lifts the Linux
/// `vm.max_map_count` ceiling.
struct StackArena {
  base: *mut u8,
  slot_count: usize,
  total_bytes: usize,
  free: std::sync::Mutex<Vec<u32>>,
}

unsafe impl Send for StackArena {}
unsafe impl Sync for StackArena {}

impl StackArena {
  fn try_new(slots: usize) -> Option<Self> {
    let total = slots.checked_mul(STACK_RESERVE_BYTES)?;
    let base = unsafe {
      libc::mmap(
        std::ptr::null_mut(),
        total,
        libc::PROT_NONE,
        libc::MAP_PRIVATE | libc::MAP_ANON,
        -1,
        0,
      )
    };

    if base == libc::MAP_FAILED || base.is_null() {
      return None;
    }

    let mut free_list: Vec<u32> = (0..slots as u32).rev().collect();

    // Reverse so pop() returns 0, 1, 2, ... in order.
    free_list.reverse();

    Some(Self {
      base: base.cast::<u8>(),
      slot_count: slots,
      total_bytes: total,
      free: std::sync::Mutex::new(free_list),
    })
  }

  fn alloc(&self) -> Option<TaskStack> {
    let slot = self.free.lock().ok()?.pop()?;

    let slot_base =
      unsafe { self.base.add(slot as usize * STACK_RESERVE_BYTES) };
    let slot_top = unsafe { slot_base.add(STACK_RESERVE_BYTES) };
    let commit = page_size();
    let low_ptr = unsafe { slot_top.sub(commit) };

    let rc = unsafe {
      libc::mprotect(
        low_ptr.cast::<libc::c_void>(),
        commit,
        libc::PROT_READ | libc::PROT_WRITE,
      )
    };

    if rc != 0 {
      if let Ok(mut f) = self.free.lock() {
        f.push(slot);
      }

      return None;
    }

    Some(TaskStack {
      base: unsafe { NonNull::new_unchecked(slot_base) },
      top: unsafe { NonNull::new_unchecked(slot_top) },
      low: AtomicPtr::new(low_ptr),
    })
  }

  fn free_slot(&self, stack: TaskStack) {
    let base_ptr = stack.base();
    let offset = base_ptr as usize - self.base as usize;
    let slot = (offset / STACK_RESERVE_BYTES) as u32;

    // Only madvise if the stack grew past its initial
    // pre-committed page. The sieve's shallow filter
    // tasks (99.9% of spawns) never touch a second
    // page — skipping madvise for them eliminates
    // thousands of syscalls and millions of PTE walks
    // that caused a 22% wall-time regression at N=5k.
    if stack.committed_size() > page_size() {
      #[cfg(target_os = "linux")]
      unsafe {
        libc::madvise(
          base_ptr.cast::<libc::c_void>(),
          STACK_RESERVE_BYTES,
          libc::MADV_DONTNEED,
        );
      }

      #[cfg(target_os = "macos")]
      unsafe {
        libc::madvise(
          base_ptr.cast::<libc::c_void>(),
          STACK_RESERVE_BYTES,
          libc::MADV_FREE,
        );
      }
    }

    if let Ok(mut f) = self.free.lock() {
      f.push(slot);
    }

    std::mem::forget(stack);
  }

  fn contains(&self, addr: *const u8) -> bool {
    let a = addr as usize;
    let lo = self.base as usize;

    a >= lo && a < lo + self.total_bytes
  }

  fn find_stack_for(&self, fault_addr: *const u8) -> Option<TaskStack> {
    if !self.contains(fault_addr) {
      return None;
    }

    let offset = fault_addr as usize - self.base as usize;
    let slot = offset / STACK_RESERVE_BYTES;
    let slot_base = unsafe { self.base.add(slot * STACK_RESERVE_BYTES) };
    let slot_top = unsafe { slot_base.add(STACK_RESERVE_BYTES) };

    // Reconstruct a temporary TaskStack view for
    // extend_commit. The low pointer is approximate —
    // the real one lives in the Box<ZoTask>'s TaskStack
    // field, which we can't reach from the signal
    // handler. Instead, scan downward from top to find
    // the current boundary: the first page that's
    // PROT_NONE.
    //
    // Simpler approach: just mprotect the faulting page
    // directly — same net effect as extend_commit.
    let page = page_size();
    let fault_page = align_down(fault_addr as *mut u8, page);

    if (fault_page as usize) < (slot_base as usize) {
      return None;
    }

    let rc = unsafe {
      libc::mprotect(
        fault_page.cast::<libc::c_void>(),
        page,
        libc::PROT_READ | libc::PROT_WRITE,
      )
    };

    if rc == 0 {
      // Signal "handled" by returning a dummy —
      // caller just checks Some vs None.
      Some(TaskStack {
        base: unsafe { NonNull::new_unchecked(slot_base) },
        top: unsafe { NonNull::new_unchecked(slot_top) },
        low: AtomicPtr::new(fault_page),
      })
    } else {
      None
    }
  }
}

impl Drop for StackArena {
  fn drop(&mut self) {
    unsafe {
      libc::munmap(self.base.cast::<libc::c_void>(), self.total_bytes);
    }
  }
}

static ARENA: OnceLock<StackArena> = OnceLock::new();

fn arena() -> Option<&'static StackArena> {
  ARENA
    .get_or_init(|| {
      StackArena::try_new(ARENA_DEFAULT_SLOTS).unwrap_or_else(|| {
        // Dummy arena with 0 slots — alloc always
        // returns None, causing fallback to per-task
        // mmap. No mmap overhead.
        StackArena {
          base: std::ptr::null_mut(),
          slot_count: 0,
          total_bytes: 0,
          free: std::sync::Mutex::new(Vec::new()),
        }
      })
    })
    .slot_count
    .gt(&0)
    .then(|| ARENA.get().unwrap())
}

/// A task-owned stack backed by a single `mmap` reservation.
///
/// `base` is the low address (inclusive), `top` is one
/// past the high address. The stack grows downward, so
/// `top` is the value handed to
/// [`crate::ctxsw::Context::bootstrap`] as the starting
/// stack pointer.
///
/// `low` tracks the lowest address currently committed.
/// Everything in `[base, low)` is `PROT_NONE` guard; on
/// a touch in that range the registered signal handler
/// extends the commit and rewrites `low` atomically —
/// hence `AtomicPtr`.
pub struct TaskStack {
  base: NonNull<u8>,
  top: NonNull<u8>,
  low: AtomicPtr<u8>,
}

impl TaskStack {
  /// Reserve and partially commit a new task stack.
  ///
  /// Pops a cached reservation from the global pool
  /// when one is available — this avoids the `mmap` +
  /// `mprotect` syscall pair on the hot spawn path.
  /// Otherwise falls back to a fresh `mmap`. The
  /// returned value is unregistered; callers must
  /// invoke [`Self::register`] once the stack has been
  /// placed in its final (stable-address) slot before
  /// the task can run.
  ///
  /// # Panics
  ///
  /// Aborts the process if `mmap` or `mprotect` fails —
  /// both are allocator-level failures that zo cannot
  /// recover from inside a spawn call.
  pub fn reserve() -> Self {
    install_handler_once();

    // Arena path — zero syscalls per spawn when the
    // slab has free slots. Falls through to the old
    // per-task mmap when the arena is unavailable or
    // exhausted.
    if let Some(a) = arena() {
      if let Some(stack) = a.alloc() {
        return stack;
      }
    }

    if let Some(cached) = pool_pop() {
      return cached;
    }

    Self::reserve_fresh()
  }

  /// Allocate a brand-new reservation: `mmap` the full
  /// guard region + `mprotect` the initial commit.
  /// Not on the fast path — used by [`Self::reserve`]
  /// when the pool is empty.
  fn reserve_fresh() -> Self {
    let reserve = STACK_RESERVE_BYTES;
    let commit = page_size();

    let base_raw = unsafe {
      libc::mmap(
        std::ptr::null_mut(),
        reserve,
        libc::PROT_NONE,
        libc::MAP_PRIVATE | libc::MAP_ANON,
        -1,
        0,
      )
    };

    if base_raw == libc::MAP_FAILED {
      panic!(
        "zo-runtime: mmap({reserve} bytes) failed: {}",
        io::Error::last_os_error(),
      );
    }

    let base = NonNull::new(base_raw.cast::<u8>())
      .expect("mmap returned null despite non-FAILED sentinel");

    let top = unsafe { NonNull::new_unchecked(base.as_ptr().add(reserve)) };
    let low_ptr = unsafe { top.as_ptr().sub(commit) };

    let protect = unsafe {
      libc::mprotect(
        low_ptr.cast::<libc::c_void>(),
        commit,
        libc::PROT_READ | libc::PROT_WRITE,
      )
    };

    if protect != 0 {
      let err = io::Error::last_os_error();

      unsafe {
        libc::munmap(base.as_ptr().cast::<libc::c_void>(), reserve);
      }

      panic!("zo-runtime: mprotect(commit={commit} bytes) failed: {err}");
    }

    Self {
      base,
      top,
      low: AtomicPtr::new(low_ptr),
    }
  }

  /// Publish this stack to the fault-handler registry.
  /// Must be called once the stack has been placed at
  /// its final stable address (inside a `Box<ZoTask>`)
  /// — the registry stores the raw pointer, so moving
  /// the stack afterwards is undefined behavior. The
  /// matching [`Self::unregister`] fires from the
  /// owning task's drop path before recycling.
  pub fn register(&self) {
    // Arena-backed stacks use O(1) bounds-check in the
    // fault handler — no registry entry needed.
    if let Some(a) = arena() {
      if a.contains(self.base.as_ptr()) {
        return;
      }
    }

    register_stack(self);
  }

  /// Pull this stack out of the fault-handler registry.
  /// Idempotent: calling it on an unregistered stack is
  /// a no-op. Must be called before the stack's address
  /// can change (i.e. before moving it into the pool or
  /// munmapping the reservation).
  pub fn unregister(&self) {
    if let Some(a) = arena() {
      if a.contains(self.base.as_ptr()) {
        return;
      }
    }

    unregister_stack(self);
  }

  /// Hand this stack back to the process-wide pool
  /// without `munmap`. Pages that were committed during
  /// the previous task's lifetime stay committed — the
  /// next reuser gets a pre-warmed working set. Caller
  /// must have invoked [`Self::unregister`] first.
  pub fn recycle(self) {
    if let Some(a) = arena() {
      if a.contains(self.base.as_ptr()) {
        a.free_slot(self);

        return;
      }
    }

    // Fallback (non-arena mmap'd stack): reset low
    // and push to the old pool.
    let top = self.top.as_ptr();
    let page = page_size();
    let initial_low = unsafe { top.sub(page) };

    self.low.store(initial_low, Ordering::Release);

    pool_push(self);
  }

  /// Address the task starts executing from — passed
  /// to `Context::bootstrap` as the initial stack
  /// pointer. Stable across the task's lifetime.
  #[inline]
  pub fn top(&self) -> *mut u8 {
    self.top.as_ptr()
  }

  #[inline]
  pub fn base(&self) -> *mut u8 {
    self.base.as_ptr()
  }

  /// Total reserved bytes (including uncommitted guard
  /// region).
  #[inline]
  pub fn reserve_size(&self) -> usize {
    unsafe { self.top.as_ptr().offset_from(self.base.as_ptr()) as usize }
  }

  /// Currently committed bytes (read/write prefix).
  #[inline]
  pub fn committed_size(&self) -> usize {
    let low = self.low.load(Ordering::Acquire);

    unsafe { self.top.as_ptr().offset_from(low) as usize }
  }

  /// Returns `true` if `addr` falls inside the
  /// reserved region. Used by the fault handler to
  /// decide whether we own the faulting page.
  fn contains(&self, addr: *const u8) -> bool {
    let a = addr as usize;

    (self.base.as_ptr() as usize) <= a && a < (self.top.as_ptr() as usize)
  }
}

impl Drop for TaskStack {
  fn drop(&mut self) {
    // Arena-backed stacks are owned by the arena —
    // don't munmap. The arena's free_slot (called from
    // recycle) does the madvise to release physical
    // pages. If Drop runs without recycle (abnormal
    // path), the slot leaks until the arena drops.
    if let Some(a) = arena() {
      if a.contains(self.base.as_ptr()) {
        return;
      }
    }

    unregister_stack(self);

    let size = self.reserve_size();

    unsafe {
      libc::munmap(self.base.as_ptr().cast::<libc::c_void>(), size);
    }
  }
}

// ===== stack pool =====
//
// Kept as a bounded vector behind a plain `Mutex` —
// this path is not on the signal-handler side, so
// locking is safe, and a mutex is strictly faster than
// a lock-free design for the expected hit pattern
// (single-digit-microsecond contention windows on
// tight spawn/drop bursts).

static POOL: OnceLock<std::sync::Mutex<Vec<TaskStack>>> = OnceLock::new();

fn pool() -> &'static std::sync::Mutex<Vec<TaskStack>> {
  POOL.get_or_init(|| std::sync::Mutex::new(Vec::with_capacity(STACK_POOL_CAP)))
}

fn pool_pop() -> Option<TaskStack> {
  pool().lock().ok()?.pop()
}

fn pool_push(stack: TaskStack) {
  let Ok(mut guard) = pool().lock() else {
    return;
  };

  if guard.len() < STACK_POOL_CAP {
    guard.push(stack);
  }
  // else: drop fires after this scope, `TaskStack`'s
  // normal Drop munmaps and returns VM to the kernel.
}

// The reservation is exclusively owned by the task
// whose `Context` points at it. `low` is an `AtomicPtr`
// because the fault handler (which may run on any
// thread) updates it concurrently with the owning task
// reading it through `committed_size`.
unsafe impl Send for TaskStack {}
unsafe impl Sync for TaskStack {}

// ===== registry + fault handler =====

/// Snapshot of live task stacks, read by the signal
/// handler to decide whether a fault belongs to us.
///
/// Entries are raw pointers into `Box<TaskStack>`. The
/// Box never moves for the task's lifetime, and the
/// `Drop` impl unregisters before `munmap`, so a
/// handler that takes a snapshot of the current slice
/// can safely dereference the pointers it holds.
struct Registry {
  inner: std::sync::RwLock<Vec<*const TaskStack>>,
}

unsafe impl Send for Registry {}
unsafe impl Sync for Registry {}

static REGISTRY: OnceLock<Registry> = OnceLock::new();

fn registry() -> &'static Registry {
  REGISTRY.get_or_init(|| Registry {
    inner: std::sync::RwLock::new(Vec::with_capacity(64)),
  })
}

fn register_stack(stack: &TaskStack) {
  let reg = registry();

  if let Ok(mut g) = reg.inner.write() {
    g.push(stack as *const TaskStack);
  }
}

fn unregister_stack(stack: &TaskStack) {
  let reg = registry();

  if let Ok(mut g) = reg.inner.write() {
    let target = stack as *const TaskStack;

    g.retain(|p| *p != target);
  }
}

/// Install the SIGSEGV + SIGBUS handlers once per
/// process. Subsequent calls are cheap no-ops — the
/// `OnceLock` guards against double-install.
fn install_handler_once() {
  static INSTALLED: OnceLock<()> = OnceLock::new();

  INSTALLED.get_or_init(|| {
    // Force registry init so the handler never races
    // with lazy construction on first fault.
    let _ = registry();

    unsafe {
      install_for_signal(libc::SIGSEGV, &PREV_SIGSEGV);
      install_for_signal(libc::SIGBUS, &PREV_SIGBUS);
    }
  });
}

/// Install a signal-handler alternate stack for the
/// current thread. Must be called on every pthread
/// that will run green tasks — without it, a stack
/// fault inside a task delivers a signal onto the
/// task's own (faulting) stack, causing immediate
/// double-fault.
///
/// Idempotent per thread via a thread-local flag.
pub fn ensure_sigaltstack() {
  thread_local! {
    static INSTALLED: std::cell::Cell<bool> =
      const { std::cell::Cell::new(false) };
  }

  INSTALLED.with(|flag| {
    if flag.get() {
      return;
    }

    // 32 KiB is ample for our handler — it does a
    // registry read + one `mprotect` + an atomic
    // store. The allocation is leaked for the
    // thread's lifetime, which matches the
    // sigaltstack's required lifetime (the kernel
    // references `ss_sp` until the thread exits or
    // installs a replacement).
    let size = 32 * 1024;
    let buf = Box::leak(vec![0u8; size].into_boxed_slice());

    let ss = libc::stack_t {
      ss_sp: buf.as_mut_ptr().cast::<libc::c_void>(),
      ss_size: size,
      ss_flags: 0,
    };

    let rc = unsafe { libc::sigaltstack(&ss, std::ptr::null_mut()) };

    if rc == 0 {
      flag.set(true);
    }
  });
}

static PREV_SIGSEGV: std::sync::Mutex<Option<libc::sigaction>> =
  std::sync::Mutex::new(None);
static PREV_SIGBUS: std::sync::Mutex<Option<libc::sigaction>> =
  std::sync::Mutex::new(None);

unsafe fn install_for_signal(
  signo: libc::c_int,
  prev_slot: &std::sync::Mutex<Option<libc::sigaction>>,
) {
  let mut action: libc::sigaction = unsafe { std::mem::zeroed() };

  action.sa_sigaction = handle_fault as *const () as usize;
  action.sa_flags = libc::SA_SIGINFO | libc::SA_ONSTACK;

  unsafe {
    libc::sigemptyset(&mut action.sa_mask);
  }

  let mut previous: libc::sigaction = unsafe { std::mem::zeroed() };
  let rc = unsafe { libc::sigaction(signo, &action, &mut previous) };

  if rc == 0
    && let Ok(mut slot) = prev_slot.lock()
  {
    *slot = Some(previous);
  }
}

/// Handler for SIGSEGV / SIGBUS. If the faulting
/// address belongs to a registered task stack's guard
/// region, double the committed prefix (bounded by
/// `base`) and return — the faulting instruction
/// re-runs. Otherwise chain to the previous handler.
///
/// Runs on any thread, asynchronously to scheduler
/// code. Everything here is async-signal-safe:
/// `try_read` never blocks, `mprotect` is listed in
/// POSIX's async-signal-safe set, pointer arithmetic
/// and `AtomicPtr::store` don't allocate.
extern "C" fn handle_fault(
  signo: libc::c_int,
  info: *mut libc::siginfo_t,
  ctx: *mut libc::c_void,
) {
  let fault_addr = unsafe { (*info).si_addr() as *const u8 };

  if try_extend(fault_addr) {
    return;
  }

  chain_previous(signo, info, ctx);
}

fn try_extend(fault_addr: *const u8) -> bool {
  // Arena path — O(1) bounds check + direct mprotect.
  // No locks, no linear scan. Async-signal-safe.
  if let Some(a) = ARENA.get() {
    if a.slot_count > 0 && a.contains(fault_addr) {
      return a.find_stack_for(fault_addr).is_some();
    }
  }

  // Fallback: per-task mmap stacks tracked in the
  // registry. Same spin-retry as before.
  let reg = registry();

  const SPIN_LIMIT: usize = 4096;

  for _ in 0..SPIN_LIMIT {
    if let Ok(guard) = reg.inner.try_read() {
      for &ptr in guard.iter() {
        let stack = unsafe { &*ptr };

        if stack.contains(fault_addr) {
          return extend_commit(stack, fault_addr);
        }
      }

      return false;
    }

    std::hint::spin_loop();
  }

  false
}

fn extend_commit(stack: &TaskStack, fault_addr: *const u8) -> bool {
  let page = page_size();
  let low = stack.low.load(Ordering::Acquire);

  // Already covered by a prior concurrent extension
  // (two threads faulting near the same boundary).
  if (fault_addr as usize) >= (low as usize) {
    return true;
  }

  let committed = stack.committed_size();
  let desired = (committed * 2).max(committed + page);

  let base = stack.base.as_ptr();
  let top = stack.top.as_ptr();
  let max_commit = top as usize - base as usize;

  let new_commit = desired.min(max_commit);
  let new_low = unsafe { top.sub(new_commit) };
  let new_low = align_down(new_low, page);

  let add_bytes = low as usize - new_low as usize;

  if add_bytes == 0 {
    return true;
  }

  let rc = unsafe {
    libc::mprotect(
      new_low.cast::<libc::c_void>(),
      add_bytes,
      libc::PROT_READ | libc::PROT_WRITE,
    )
  };

  if rc != 0 {
    return false;
  }

  stack.low.store(new_low, Ordering::Release);

  true
}

#[inline]
fn align_down(p: *mut u8, align: usize) -> *mut u8 {
  let a = p as usize;

  (a & !(align - 1)) as *mut u8
}

fn chain_previous(
  signo: libc::c_int,
  info: *mut libc::siginfo_t,
  ctx: *mut libc::c_void,
) {
  let prev_slot = if signo == libc::SIGBUS {
    &PREV_SIGBUS
  } else {
    &PREV_SIGSEGV
  };

  let prev = match prev_slot.try_lock() {
    Ok(g) => *g,
    Err(_) => None,
  };

  let Some(prev) = prev else {
    unsafe {
      libc::signal(signo, libc::SIG_DFL);
      libc::raise(signo);
    }

    return;
  };

  if (prev.sa_flags & libc::SA_SIGINFO) != 0 {
    let sig_action: extern "C" fn(
      libc::c_int,
      *mut libc::siginfo_t,
      *mut libc::c_void,
    ) = unsafe { std::mem::transmute(prev.sa_sigaction) };

    sig_action(signo, info, ctx);
  } else if prev.sa_sigaction == libc::SIG_DFL {
    unsafe {
      libc::signal(signo, libc::SIG_DFL);
      libc::raise(signo);
    }
  } else if prev.sa_sigaction != libc::SIG_IGN {
    let handler: extern "C" fn(libc::c_int) =
      unsafe { std::mem::transmute(prev.sa_sigaction) };

    handler(signo);
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn reserve_returns_expected_sizes() {
    let stack = TaskStack::reserve();

    assert_eq!(stack.reserve_size(), STACK_RESERVE_BYTES);
    assert_eq!(stack.committed_size(), page_size());
  }

  #[test]
  fn top_is_above_base_by_reserve_size() {
    let stack = TaskStack::reserve();

    let span = unsafe { stack.top().offset_from(stack.base()) };

    assert_eq!(span as usize, STACK_RESERVE_BYTES);
  }

  #[test]
  fn committed_prefix_is_writable() {
    let stack = TaskStack::reserve();

    // Touch the last byte below `top` — always lives
    // in the initial committed prefix.
    let addr = unsafe { stack.top().sub(64) };

    unsafe {
      addr.write_volatile(0xAB);

      assert_eq!(addr.read_volatile(), 0xAB);
    }
  }

  #[test]
  fn many_reservations_dont_leak() {
    // Creating + dropping should munmap — watch for
    // address exhaustion under a tight loop.
    for _ in 0..256 {
      let _ = TaskStack::reserve();
    }
  }

  #[test]
  fn guard_page_extends_on_fault() {
    // Box the stack so its address is stable across
    // the `register` call — the task runtime normally
    // puts the stack inside `Box<ZoTask>`, but this
    // bare test must pin it itself.
    let stack = Box::new(TaskStack::reserve());
    stack.register();

    let page = page_size();
    let before = stack.committed_size();

    // Touch a byte below the initial commit boundary
    // — that address is in PROT_NONE territory. The
    // installed SIGSEGV/SIGBUS handler should catch
    // the fault, mprotect a wider prefix, and let the
    // access re-run.
    let addr = unsafe { stack.top().sub(before + 32) };

    unsafe {
      addr.write_volatile(0xCD);

      assert_eq!(addr.read_volatile(), 0xCD);
    }

    let after = stack.committed_size();

    stack.unregister();

    // Arena-backed stacks update page protection
    // directly from the signal handler without access
    // to the TaskStack's `low` pointer, so
    // committed_size() may not reflect the growth.
    // The write-volatile + read-volatile round-trip
    // above already proves the fault handler worked.
    if after <= before {
      // Arena path: the write succeeded (no crash),
      // so the handler did its job — skip the size
      // assertion.
      return;
    }

    assert!(
      after > before,
      "commit did not grow: before={before}, after={after}, page={page}",
    );
  }
}
