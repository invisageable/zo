//! Context-switch primitive for green tasks.
//!
//! A `Context` captures the subset of CPU state that
//! survives a function call boundary on the platform's
//! calling convention: on AAPCS64 (aarch64) that's the
//! 10 callee-save GPRs `x19..x28`, frame / link regs
//! `x29` / `x30`, stack pointer, and 8 callee-save FP
//! regs `d8..d15`. On SysV AMD64 (x86_64) it's the 6
//! callee-save GPRs `rbx`, `rbp`, `r12..r15`, plus the
//! stack pointer; the return address lives on the stack
//! itself. Everything else is caller-save — the
//! compiler already spills it at the call site, so a
//! voluntary yield doesn't need to touch those regs.
//!
//! `ctx_switch(from, to)` saves the current CPU state
//! into `*from`, loads from `*to`, and returns via the
//! target's stored return path. Voluntary yield only —
//! a preemption-capable variant would need the full
//! register file.
//!
//! The companion `zo_task_entry_trampoline` is the
//! bootstrap entry for a freshly-minted Context: it
//! pulls `entry_fn` + `arg` out of callee-save slots,
//! moves `arg` into the first-argument register, and
//! calls `entry_fn`. If `entry_fn` returns, the
//! trampoline aborts — a task body is expected to
//! ctx_switch out, not return.
//!
//! Platforms: aarch64-apple-darwin and x86_64-*-linux.
//! Other triples compile the module as-is (see the
//! compile_error at the bottom) — the code is gated by
//! per-target cfg so unrelated crates can still build.
//!
//! ```sh
//! cargo test -p zo-runtime ctxsw
//! ```

use std::arch::global_asm;

// ===== Context layout =====

/// Saved CPU state snapshot for aarch64 / AAPCS64 —
/// exactly the callee-save set. Layout is `#[repr(C)]`
/// so the hand-written asm below can index fields by
/// absolute byte offset without drift.
#[cfg(target_arch = "aarch64")]
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Context {
  /// `x19..x28` — 10 callee-save general-purpose regs.
  pub gp_regs: [u64; 10],
  /// `x29` — frame pointer.
  pub fp: u64,
  /// `x30` — link register (return address).
  pub lr: u64,
  /// Stack pointer.
  pub sp: u64,
  /// `d8..d15` — 8 callee-save FP / SIMD regs.
  pub fp_regs: [u64; 8],
}

/// Saved CPU state snapshot for SysV AMD64 (x86_64).
/// The 6 callee-save GPRs plus the stack pointer;
/// return address survives on the stack itself, so it
/// doesn't need its own slot. `#[repr(C)]` locks the
/// offsets for the asm block.
#[cfg(target_arch = "x86_64")]
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Context {
  /// `rbx`, `rbp`, `r12`, `r13`, `r14`, `r15`.
  pub gp_regs: [u64; 6],
  /// Stack pointer.
  pub rsp: u64,
}

// Hard-coded offsets the asm relies on. If the struct
// layout shifts, the compile fails here before anything
// runs the wrong bytes.
#[cfg(target_arch = "aarch64")]
const _: () = {
  assert!(core::mem::size_of::<Context>() == 168);
  assert!(core::mem::align_of::<Context>() == 8);
};

#[cfg(target_arch = "x86_64")]
const _: () = {
  assert!(core::mem::size_of::<Context>() == 56);
  assert!(core::mem::align_of::<Context>() == 8);
};

impl Context {
  /// All-zero context — safe to CONSTRUCT but not safe
  /// to switch INTO directly. Caller must populate via
  /// [`Context::bootstrap`] before passing this as the
  /// `to` argument of [`ctx_switch`].
  #[cfg(target_arch = "aarch64")]
  pub const fn zeroed() -> Self {
    Self {
      gp_regs: [0; 10],
      fp: 0,
      lr: 0,
      sp: 0,
      fp_regs: [0; 8],
    }
  }

  #[cfg(target_arch = "x86_64")]
  pub const fn zeroed() -> Self {
    Self {
      gp_regs: [0; 6],
      rsp: 0,
    }
  }

  /// Prepare `self` so the next `ctx_switch(from, self)`
  /// lands in `entry(arg)`.
  ///
  /// - `stack_top` is the HIGHEST address of the task's
  ///   stack region. Stacks grow downward, so this is
  ///   the byte AFTER the last usable byte.
  /// - `entry` is the task's real body — `extern "C"`
  ///   so the trampoline's arg-register setup matches
  ///   the C calling convention.
  /// - `arg` is an opaque u64 the trampoline moves into
  ///   the first-argument register before branching.
  ///
  /// Subsequent switches resume `entry` at whatever
  /// instruction it last yielded from — the trampoline
  /// is one-shot.
  #[cfg(target_arch = "aarch64")]
  pub fn bootstrap(
    &mut self,
    stack_top: *mut u8,
    entry: extern "C-unwind" fn(u64),
    arg: u64,
  ) {
    // x19 <- entry, x20 <- arg. The trampoline pulls
    // them out of callee-save slots after the
    // ctx_switch load restored them. Function pointers
    // go through `*const ()` on the way to u64 so
    // clippy's `function_casts_as_integer` lint stays
    // happy — the intent is "opaque address", not
    // "numeric value".
    self.gp_regs[0] = entry as *const () as u64;
    self.gp_regs[1] = arg;

    self.lr = zo_task_entry_trampoline as *const () as u64;

    // 16-byte align SP (AAPCS64 requirement at call
    // boundaries). `& !15` rounds DOWN since stacks
    // grow toward lower addresses.
    self.sp = (stack_top as u64) & !15;
  }

  /// Prepare `self` so the next `ctx_switch(from, self)`
  /// lands in `entry(arg)` on x86_64.
  ///
  /// SysV AMD64 has no `lr` register — the return
  /// address lives on the stack. Bootstrap simulates
  /// "someone called the trampoline" by writing the
  /// trampoline's address at `stack_top - 8` and setting
  /// `rsp` to that slot. When `ctx_switch`'s `ret`
  /// fires, it pops that address off the stack and
  /// jumps to the trampoline. After the pop, `rsp`
  /// equals the original `stack_top`, which must be
  /// 16-byte aligned per the SysV ABI's entry
  /// contract.
  #[cfg(target_arch = "x86_64")]
  pub fn bootstrap(
    &mut self,
    stack_top: *mut u8,
    entry: extern "C-unwind" fn(u64),
    arg: u64,
  ) {
    // rbx <- entry, r12 <- arg. The trampoline pulls
    // them out of callee-save slots after ctx_switch
    // restores them.
    self.gp_regs[0] = entry as *const () as u64;
    self.gp_regs[2] = arg;

    // 16-byte align `stack_top` downward, then reserve
    // one 8-byte slot at the top for the trampoline's
    // return address. After the trampoline's `ret` /
    // `pop` sequence, rsp becomes the 16-byte-aligned
    // boundary, matching the SysV AMD64 "entry at a
    // call site" contract.
    let aligned_top = (stack_top as u64) & !15;
    let ret_slot = aligned_top - 8;

    unsafe {
      (ret_slot as *mut u64)
        .write(zo_task_entry_trampoline as *const () as u64);
    }

    self.rsp = ret_slot;
  }
}

// ===== External symbols defined in the asm block =====

unsafe extern "C" {
  /// Save current CPU state into `*from`, load from
  /// `*to`, return to the target's saved return path.
  ///
  /// # Safety
  ///
  /// - `from` and `to` must be valid, distinct,
  ///   non-null pointers to writable/readable
  ///   `Context` instances respectively.
  /// - `to` must have been populated by a previous
  ///   `ctx_switch(other, to)` OR by
  ///   [`Context::bootstrap`]. A zeroed context is not
  ///   a valid `to`.
  /// - The stack `to`'s saved stack pointer points at
  ///   must be alive and unreclaimed for the entire
  ///   time `to` remains a switchable target.
  pub fn ctx_switch(from: *mut Context, to: *mut Context);

  /// Bootstrap trampoline — not callable directly;
  /// entered only through a ctx_switch into a
  /// bootstrap-initialized context.
  fn zo_task_entry_trampoline();
}

// ===== aarch64 assembly =====
//
// One asm block, label prefix chosen by target_os.
// Darwin mangles C symbols with a leading underscore;
// Linux does not. The Rust-side `extern "C"` binding
// resolves the right form at link time.

#[cfg(all(target_arch = "aarch64", target_os = "macos"))]
global_asm!(
  ".global _ctx_switch",
  ".p2align 2",
  "_ctx_switch:",
  "    stp x19, x20, [x0, #0]",
  "    stp x21, x22, [x0, #16]",
  "    stp x23, x24, [x0, #32]",
  "    stp x25, x26, [x0, #48]",
  "    stp x27, x28, [x0, #64]",
  "    stp x29, x30, [x0, #80]",
  "    mov x9, sp",
  "    str x9, [x0, #96]",
  "    stp d8, d9,   [x0, #104]",
  "    stp d10, d11, [x0, #120]",
  "    stp d12, d13, [x0, #136]",
  "    stp d14, d15, [x0, #152]",
  "    ldp x19, x20, [x1, #0]",
  "    ldp x21, x22, [x1, #16]",
  "    ldp x23, x24, [x1, #32]",
  "    ldp x25, x26, [x1, #48]",
  "    ldp x27, x28, [x1, #64]",
  "    ldp x29, x30, [x1, #80]",
  "    ldr x9, [x1, #96]",
  "    mov sp, x9",
  "    ldp d8, d9,   [x1, #104]",
  "    ldp d10, d11, [x1, #120]",
  "    ldp d12, d13, [x1, #136]",
  "    ldp d14, d15, [x1, #152]",
  "    ret",
  ".global _zo_task_entry_trampoline",
  ".p2align 2",
  "_zo_task_entry_trampoline:",
  "    mov x0, x20",
  "    blr x19",
  "    bl _abort",
);

#[cfg(all(target_arch = "aarch64", target_os = "linux"))]
global_asm!(
  ".global ctx_switch",
  ".p2align 2",
  "ctx_switch:",
  "    stp x19, x20, [x0, #0]",
  "    stp x21, x22, [x0, #16]",
  "    stp x23, x24, [x0, #32]",
  "    stp x25, x26, [x0, #48]",
  "    stp x27, x28, [x0, #64]",
  "    stp x29, x30, [x0, #80]",
  "    mov x9, sp",
  "    str x9, [x0, #96]",
  "    stp d8, d9,   [x0, #104]",
  "    stp d10, d11, [x0, #120]",
  "    stp d12, d13, [x0, #136]",
  "    stp d14, d15, [x0, #152]",
  "    ldp x19, x20, [x1, #0]",
  "    ldp x21, x22, [x1, #16]",
  "    ldp x23, x24, [x1, #32]",
  "    ldp x25, x26, [x1, #48]",
  "    ldp x27, x28, [x1, #64]",
  "    ldp x29, x30, [x1, #80]",
  "    ldr x9, [x1, #96]",
  "    mov sp, x9",
  "    ldp d8, d9,   [x1, #104]",
  "    ldp d10, d11, [x1, #120]",
  "    ldp d12, d13, [x1, #136]",
  "    ldp d14, d15, [x1, #152]",
  "    ret",
  ".global zo_task_entry_trampoline",
  ".p2align 2",
  "zo_task_entry_trampoline:",
  "    mov x0, x20",
  "    blr x19",
  "    bl abort",
);

// ===== x86_64 assembly =====
//
// SysV AMD64 calling convention. Args `from` / `to`
// come in rdi / rsi. Callee-save GPRs are rbx, rbp,
// r12..r15. Return address survives on the stack
// (popped by `ret`). No callee-save FP regs in SysV.
//
// Context slot layout (6 × u64 + rsp):
//   [0]=rbx [8]=rbp [16]=r12 [24]=r13 [32]=r14 [40]=r15
//   [48]=rsp

#[cfg(all(target_arch = "x86_64", target_os = "linux"))]
global_asm!(
  ".global ctx_switch",
  ".p2align 4",
  "ctx_switch:",
  "    mov [rdi + 0],  rbx",
  "    mov [rdi + 8],  rbp",
  "    mov [rdi + 16], r12",
  "    mov [rdi + 24], r13",
  "    mov [rdi + 32], r14",
  "    mov [rdi + 40], r15",
  "    mov [rdi + 48], rsp",
  "    mov rbx, [rsi + 0]",
  "    mov rbp, [rsi + 8]",
  "    mov r12, [rsi + 16]",
  "    mov r13, [rsi + 24]",
  "    mov r14, [rsi + 32]",
  "    mov r15, [rsi + 40]",
  "    mov rsp, [rsi + 48]",
  "    ret",
  ".global zo_task_entry_trampoline",
  ".p2align 4",
  "zo_task_entry_trampoline:",
  "    mov rdi, r12",
  "    call rbx",
  "    call abort",
);

#[cfg(all(target_arch = "x86_64", target_os = "macos"))]
global_asm!(
  ".global _ctx_switch",
  ".p2align 4",
  "_ctx_switch:",
  "    mov [rdi + 0],  rbx",
  "    mov [rdi + 8],  rbp",
  "    mov [rdi + 16], r12",
  "    mov [rdi + 24], r13",
  "    mov [rdi + 32], r14",
  "    mov [rdi + 40], r15",
  "    mov [rdi + 48], rsp",
  "    mov rbx, [rsi + 0]",
  "    mov rbp, [rsi + 8]",
  "    mov r12, [rsi + 16]",
  "    mov r13, [rsi + 24]",
  "    mov r14, [rsi + 32]",
  "    mov r15, [rsi + 40]",
  "    mov rsp, [rsi + 48]",
  "    ret",
  ".global _zo_task_entry_trampoline",
  ".p2align 4",
  "_zo_task_entry_trampoline:",
  "    mov rdi, r12",
  "    call rbx",
  "    call _abort",
);

#[cfg(not(any(
  all(target_arch = "aarch64", target_os = "macos"),
  all(target_arch = "aarch64", target_os = "linux"),
  all(target_arch = "x86_64", target_os = "linux"),
  all(target_arch = "x86_64", target_os = "macos"),
)))]
compile_error!(
  "zo-runtime::ctxsw supports aarch64-apple-darwin, \
   aarch64-unknown-linux-*, x86_64-unknown-linux-*, \
   and x86_64-apple-darwin. Windows / other targets \
   need their own asm block."
);

// ===== Tests =====

#[cfg(test)]
mod tests {
  use super::*;

  use std::sync::atomic::{AtomicU64, Ordering};

  #[test]
  fn context_has_expected_layout() {
    #[cfg(target_arch = "aarch64")]
    {
      assert_eq!(core::mem::size_of::<Context>(), 168);
    }

    #[cfg(target_arch = "x86_64")]
    {
      assert_eq!(core::mem::size_of::<Context>(), 56);
    }

    assert_eq!(core::mem::align_of::<Context>(), 8);
  }

  #[test]
  fn zeroed_context_fields_are_all_zero() {
    let ctx = Context::zeroed();

    assert!(ctx.gp_regs.iter().all(|&v| v == 0));

    #[cfg(target_arch = "aarch64")]
    {
      assert_eq!(ctx.fp, 0);
      assert_eq!(ctx.lr, 0);
      assert_eq!(ctx.sp, 0);
      assert!(ctx.fp_regs.iter().all(|&v| v == 0));
    }

    #[cfg(target_arch = "x86_64")]
    {
      assert_eq!(ctx.rsp, 0);
    }
  }

  #[test]
  fn bootstrap_populates_entry_arg_and_stack_pointer() {
    let mut stack = vec![0u8; 4096].into_boxed_slice();
    let top = unsafe { stack.as_mut_ptr().add(stack.len()) };

    extern "C-unwind" fn never_runs(_arg: u64) {}

    let mut ctx = Context::zeroed();

    ctx.bootstrap(top, never_runs, 0xCAFEBABE);

    #[cfg(target_arch = "aarch64")]
    {
      assert_eq!(ctx.gp_regs[0], never_runs as *const () as u64);
      assert_eq!(ctx.gp_regs[1], 0xCAFEBABE);
      assert_eq!(ctx.lr, zo_task_entry_trampoline as *const () as u64);
      assert!(ctx.sp.is_multiple_of(16));
      assert!(ctx.sp <= top as u64);
      assert!(ctx.sp > stack.as_ptr() as u64);
    }

    #[cfg(target_arch = "x86_64")]
    {
      assert_eq!(ctx.gp_regs[0], never_runs as *const () as u64);
      assert_eq!(ctx.gp_regs[2], 0xCAFEBABE);
      assert!(ctx.rsp < top as u64);
      assert!(ctx.rsp > stack.as_ptr() as u64);
      // The stored retaddr should be the trampoline.
      let retaddr = unsafe { (ctx.rsp as *const u64).read() };
      assert_eq!(retaddr, zo_task_entry_trampoline as *const () as u64);
    }
  }

  // Global state ferrying pointers into the `extern "C"`
  // child entry. We can't capture by closure across the
  // ABI boundary, so we stash raw addresses here.
  static COUNTER: AtomicU64 = AtomicU64::new(0);
  static MAIN_CTX_ADDR: AtomicU64 = AtomicU64::new(0);
  static CHILD_CTX_ADDR: AtomicU64 = AtomicU64::new(0);

  extern "C-unwind" fn ping_pong_child(_arg: u64) {
    loop {
      COUNTER.fetch_add(1, Ordering::SeqCst);

      let main_ctx = MAIN_CTX_ADDR.load(Ordering::SeqCst) as *mut Context;
      let child_ctx = CHILD_CTX_ADDR.load(Ordering::SeqCst) as *mut Context;

      unsafe {
        ctx_switch(child_ctx, main_ctx);
      }
    }
  }

  #[test]
  fn ctx_switch_ping_pong_100_round_trips() {
    const STACK_SIZE: usize = 64 * 1024;

    let mut stack = vec![0u8; STACK_SIZE].into_boxed_slice();
    let stack_top = unsafe { stack.as_mut_ptr().add(STACK_SIZE) };

    let mut main_ctx = Context::zeroed();
    let mut child_ctx = Context::zeroed();

    child_ctx.bootstrap(stack_top, ping_pong_child, 0);

    MAIN_CTX_ADDR.store(&mut main_ctx as *mut _ as u64, Ordering::SeqCst);
    CHILD_CTX_ADDR.store(&mut child_ctx as *mut _ as u64, Ordering::SeqCst);

    COUNTER.store(0, Ordering::SeqCst);

    for _ in 0..100 {
      unsafe {
        ctx_switch(&mut main_ctx, &mut child_ctx);
      }
    }

    assert_eq!(COUNTER.load(Ordering::SeqCst), 100);
  }
}
