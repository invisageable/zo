//! Phase 2 of `PLAN_PREHISTORY.md` — context-switch
//! primitive.
//!
//! A `Context` captures the subset of CPU state that
//! survives a function call boundary on the platform's
//! calling convention (AAPCS64 on aarch64): the 10
//! callee-save GPRs `x19..x28`, the frame/link registers
//! `x29`/`x30`, the stack pointer, and the 8 callee-save
//! FP regs `d8..d15`. Everything else is caller-save —
//! the compiler already spills it at the `bl` site, so a
//! voluntary yield (which `ctx_switch` is) doesn't need
//! to touch those regs.
//!
//! `ctx_switch(from, to)` saves the current CPU state
//! into `*from`, loads from `*to`, and `ret`s to
//! `to.lr`. This is a voluntary yield only — the callee-
//! save set is enough because every other register was
//! already spilled by the compiler for the `bl`. For
//! preemptive context switches (signal handlers) we'd
//! need the FULL register file, which is not this
//! phase's scope.
//!
//! The companion `zo_task_entry_trampoline` is the
//! bootstrap entry for a freshly-minted Context: it
//! expects `x19 = entry_fn`, `x20 = arg`, moves the
//! arg into the first-argument register `x0`, and
//! branches to `entry_fn`.
//!
//! Platforms: aarch64 + macOS (Darwin) only for v1.
//! Linux / x86_64 lands in Phase 8 — same mechanics,
//! different symbol-prefixing and register files.
//!
//! ```sh
//! cargo test -p zo-runtime ctxsw
//! ```

use std::arch::global_asm;

// ===== Context layout (aarch64) =====

/// Saved CPU state snapshot. Exactly the callee-save set
/// for AAPCS64 — enough for voluntary yields. Layout is
/// `#[repr(C)]` so the hand-written asm below can index
/// fields by absolute byte offset without drift.
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

// Hard-coded offsets the asm relies on. If the struct
// layout shifts, the compile fails here before anything
// runs the wrong bytes. Each pair is (offset, expected).
#[cfg(target_arch = "aarch64")]
const _: () = {
  assert!(core::mem::size_of::<Context>() == 168);
  assert!(core::mem::align_of::<Context>() == 8);
};

impl Context {
  /// All-zero context — safe to CONSTRUCT but not safe to
  /// switch INTO directly. Caller must populate at least
  /// `lr` and `sp` (via [`Context::bootstrap`]) before
  /// passing this as the `to` argument of
  /// [`ctx_switch`].
  pub const fn zeroed() -> Self {
    Self {
      gp_regs: [0; 10],
      fp: 0,
      lr: 0,
      sp: 0,
      fp_regs: [0; 8],
    }
  }

  /// Prepare `self` so the next `ctx_switch(from, self)`
  /// lands in `entry(arg)`.
  ///
  /// - `stack_top` is the HIGHEST address of the task's
  ///   stack region. Stacks grow downward on aarch64 and
  ///   x86_64, so this is the byte AFTER the last usable
  ///   byte. 16-byte aligned per AAPCS64.
  /// - `entry` is the task's real body — `extern "C"` so
  ///   the trampoline's arg-register setup matches the
  ///   C calling convention.
  /// - `arg` is an opaque u64 the trampoline moves into
  ///   `x0` before branching. Typically a heap pointer
  ///   cast via `as u64`.
  ///
  /// First-switch control flow:
  /// 1. `ctx_switch` loads our `gp_regs` + `lr` + `sp`.
  /// 2. `ret` branches to our `lr` = trampoline.
  /// 3. Trampoline reads `x19` (entry) and `x20` (arg),
  ///    does `mov x0, x20; blr x19`.
  /// 4. `entry(arg)` runs in a fresh stack frame on the
  ///    provided stack.
  ///
  /// Subsequent switches resume `entry` at whatever
  /// instruction it last yielded from — the trampoline is
  /// one-shot.
  #[cfg(target_arch = "aarch64")]
  pub fn bootstrap(
    &mut self,
    stack_top: *mut u8,
    entry: extern "C" fn(u64),
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
}

// ===== External symbols defined in the asm block =====

unsafe extern "C" {
  /// Save current CPU state into `*from`, load from
  /// `*to`, return to `to.lr`.
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
  /// - The stack `to.sp` points at must be alive and
  ///   unreclaimed for the entire time `to` remains a
  ///   switchable target.
  pub fn ctx_switch(from: *mut Context, to: *mut Context);

  /// Bootstrap trampoline — not callable directly;
  /// entered only through a ctx_switch into a
  /// bootstrap-initialized context.
  fn zo_task_entry_trampoline();
}

// ===== aarch64-apple-darwin assembly =====
//
// Darwin mangles C symbols with a leading underscore;
// Rust's extern "C" symbols are `_ctx_switch` etc. at
// the linker level. Linux + x86_64 land in Phase 8.

#[cfg(all(target_arch = "aarch64", target_os = "macos"))]
global_asm!(
  // ---- ctx_switch ----
  ".global _ctx_switch",
  ".p2align 2",
  "_ctx_switch:",
  // Save callee-save GPRs x19..x28 into *from (x0).
  "    stp x19, x20, [x0, #0]",
  "    stp x21, x22, [x0, #16]",
  "    stp x23, x24, [x0, #32]",
  "    stp x25, x26, [x0, #48]",
  "    stp x27, x28, [x0, #64]",
  // Save fp (x29) + lr (x30).
  "    stp x29, x30, [x0, #80]",
  // Save sp — can't STR sp directly; bounce via x9.
  "    mov x9, sp",
  "    str x9, [x0, #96]",
  // Save callee-save FP regs d8..d15.
  "    stp d8, d9,   [x0, #104]",
  "    stp d10, d11, [x0, #120]",
  "    stp d12, d13, [x0, #136]",
  "    stp d14, d15, [x0, #152]",
  // Load new state from *to (x1).
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
  // Return to to.lr (now in x30).
  "    ret",
  // ---- zo_task_entry_trampoline ----
  //
  // Called on the first ctx_switch into a bootstrap-
  // initialized context. Reads entry_fn from x19 and
  // arg from x20 (placed there by `Context::bootstrap`),
  // moves the arg into x0 (first-arg reg), and branches
  // to the entry function.
  //
  // If `entry` ever returns, the trampoline falls
  // through to `abort()` — a task that returns without
  // ctx_switching out has no parent frame to return to,
  // so aborting is the honest failure mode.
  ".global _zo_task_entry_trampoline",
  ".p2align 2",
  "_zo_task_entry_trampoline:",
  "    mov x0, x20",
  "    blr x19",
  "    bl _abort",
);

#[cfg(not(all(target_arch = "aarch64", target_os = "macos")))]
compile_error!(
  "zo-runtime::ctxsw currently only supports \
   aarch64-apple-darwin (Phase 2 MVP). \
   Linux + x86_64 support lands in \
   PLAN_PREHISTORY Phase 8."
);

// ===== Tests =====

#[cfg(test)]
mod tests {
  use super::*;

  use std::sync::atomic::{AtomicU64, Ordering};

  #[test]
  fn context_has_expected_layout() {
    // Matches the hardcoded offsets in the asm block.
    assert_eq!(core::mem::size_of::<Context>(), 168);
    assert_eq!(core::mem::align_of::<Context>(), 8);
  }

  #[test]
  fn zeroed_context_fields_are_all_zero() {
    let ctx = Context::zeroed();

    assert!(ctx.gp_regs.iter().all(|&v| v == 0));
    assert_eq!(ctx.fp, 0);
    assert_eq!(ctx.lr, 0);
    assert_eq!(ctx.sp, 0);
    assert!(ctx.fp_regs.iter().all(|&v| v == 0));
  }

  #[test]
  fn bootstrap_populates_entry_arg_lr_sp() {
    let mut stack = vec![0u8; 4096].into_boxed_slice();
    let top = unsafe { stack.as_mut_ptr().add(stack.len()) };

    extern "C" fn never_runs(_arg: u64) {}

    let mut ctx = Context::zeroed();

    ctx.bootstrap(top, never_runs, 0xCAFEBABE);

    // gp_regs[0] carries entry, gp_regs[1] carries arg, per the
    // trampoline convention documented on `bootstrap`.
    assert_eq!(ctx.gp_regs[0], never_runs as *const () as u64);
    assert_eq!(ctx.gp_regs[1], 0xCAFEBABE);
    assert_eq!(ctx.lr, zo_task_entry_trampoline as *const () as u64);

    // SP 16-byte aligned and within the allocated range.
    assert!(ctx.sp.is_multiple_of(16));
    assert!(ctx.sp <= top as u64);
    assert!(ctx.sp > stack.as_ptr() as u64);
  }

  // Global state ferrying pointers into the `extern "C"`
  // child entry. We can't capture by closure across the
  // ABI boundary, so we stash raw addresses here.
  static COUNTER: AtomicU64 = AtomicU64::new(0);
  static MAIN_CTX_ADDR: AtomicU64 = AtomicU64::new(0);
  static CHILD_CTX_ADDR: AtomicU64 = AtomicU64::new(0);

  extern "C" fn ping_pong_child(_arg: u64) {
    // Each time we're resumed: bump the counter, yield
    // back to main. `ctx_switch` saves our state into
    // child_ctx so the next resume picks up here.
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
    // A 64 KB stack is ample for this child (it only
    // touches its own frame + the ctx_switch call).
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
