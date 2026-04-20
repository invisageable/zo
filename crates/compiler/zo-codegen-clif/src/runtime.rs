//! Libc-facing emission primitives.
//!
//! - [`ensure_libc_func`] / [`ensure_anon_data`] cache libc
//!   imports and `.rodata` blobs per module so every caller
//!   gets the same `FuncId` / `DataId`.
//! - [`emit_write_call`] emits a libc `write(fd, buf, count)`.
//! - [`emit_exit_1`] emits a Rosetta-safe `exit(1)` terminator.
//! - [`trap_and_resume`] wraps [`emit_exit_1`] with a fresh
//!   post-terminator block so callers can keep emitting insns.

use crate::context::{FunCtx, TCtx};

use cranelift::codegen::ir;
use cranelift::codegen::ir::{AbiParam, InstBuilder};
use cranelift::codegen::isa::CallConv;
use cranelift::frontend::FunctionBuilder;
use cranelift_module::{DataDescription, DataId, FuncId, Linkage, Module};

/// Declares a libc function as an `Import` exactly once per
/// module, keyed by `name`. The signature builder runs only
/// on the cache-miss path so repeated calls are a pure
/// `HashMap::get`. `cc` resolves the symbol against libc /
/// libSystem at link time — no extra `-l` flag needed, both
/// are pulled in by the default C startup.
pub(crate) fn ensure_libc_func(
  tctx: &mut TCtx<'_>,
  name: &'static str,
  build_sig: impl FnOnce(ir::Type, CallConv) -> ir::Signature,
) -> FuncId {
  if let Some(&id) = tctx.libc_funcs.get(name) {
    return id;
  }

  let call_conv = tctx.module.target_config().default_call_conv;
  let sig = build_sig(tctx.ptr_ty, call_conv);
  let id = tctx
    .module
    .declare_function(name, Linkage::Import, &sig)
    .expect("declare libc function failed");

  tctx.libc_funcs.insert(name, id);

  id
}

/// Interns an anonymous read-only data blob exactly once per
/// module, keyed by a stable label. Cache hits return the
/// stashed `DataId`; misses allocate + define and store the
/// new id. Used by `emit_io_intrinsic` for the `"\n"`,
/// `"%lld"`, `"true"`, `"false"` buffers.
pub(crate) fn ensure_anon_data(
  tctx: &mut TCtx<'_>,
  key: &'static str,
  bytes: &[u8],
) -> DataId {
  if let Some(&id) = tctx.anon_data.get(key) {
    return id;
  }

  let mut desc = DataDescription::new();

  desc.define(bytes.to_vec().into_boxed_slice());

  let id = tctx
    .module
    .declare_anonymous_data(false, false)
    .expect("declare anonymous data failed");

  tctx
    .module
    .define_data(id, &desc)
    .expect("define anonymous data failed");

  tctx.anon_data.insert(key, id);

  id
}

/// Emits a libc `write(fd, buf, count)` CLIF call. Declares
/// `write` on first use via `ensure_libc_func`.
pub(crate) fn emit_write_call(
  tctx: &mut TCtx<'_>,
  builder: &mut FunctionBuilder,
  fd: i64,
  buf: ir::Value,
  count: ir::Value,
) {
  let func_id = ensure_libc_func(tctx, "write", |ptr_ty, cc| {
    let mut sig = ir::Signature::new(cc);

    sig.params.push(AbiParam::new(ir::types::I32)); // fd
    sig.params.push(AbiParam::new(ptr_ty)); // buf
    sig.params.push(AbiParam::new(ptr_ty)); // count (size_t)
    sig.returns.push(AbiParam::new(ptr_ty)); // ssize_t

    sig
  });

  let fref = tctx.module.declare_func_in_func(func_id, builder.func);
  let fd_v = builder.ins().iconst(ir::types::I32, fd);

  builder.ins().call(fref, &[fd_v, buf, count]);
}

/// Emits `exit(1)` as a block terminator: libc call followed
/// by an unreachable `trap`.
///
/// Why `exit(1)` instead of CLIF's bare `trap`:
/// - `trap` lowers to `ud2` on x86_64. Rosetta (x86_64 →
///   arm64 on Apple Silicon) **hangs** on `ud2` rather than
///   raising `SIGILL`.
/// - `abort()` raises `SIGABRT`, which triggers macOS'
///   crash-reporter and leaves the process in a blocked
///   state pending diagnostic collection.
/// - `exit(1)` calls libc `__exit` cleanly: no signal, no
///   crash report, process terminates with code 1. Works
///   identically on x86_64-native, Linux, and Rosetta.
///
/// `exit()` is `noreturn`, so the trailing CLIF `trap` is
/// an unreachable terminator that only exists to satisfy
/// CLIF's "every block ends with a terminator" rule.
///
/// Caller-side contract: the current block is now
/// terminated. The caller MUST either switch to a fresh
/// block before emitting more insns (see `trap_and_resume`)
/// or return control to a site that will do so.
pub(crate) fn emit_exit_1(tctx: &mut TCtx<'_>, builder: &mut FunctionBuilder) {
  let fid = ensure_libc_func(tctx, "exit", |_, cc| {
    let mut sig = ir::Signature::new(cc);

    sig.params.push(AbiParam::new(ir::types::I32));

    sig
  });
  let fref = tctx.module.declare_func_in_func(fid, builder.func);
  let code = builder.ins().iconst(ir::types::I32, 1);

  builder.ins().call(fref, &[code]);

  // Unreachable block terminator — satisfies CLIF's
  // "every block ends with a terminator" rule after the
  // `exit` call, which doesn't return.
  builder.ins().trap(ir::TrapCode::user(1).unwrap());
}

/// Exits the program with code 1 and prepares a fresh block
/// for subsequent instructions.
///
/// Used by helpers that hit an unsupported / malformed case
/// and return control to a `continue`-style caller (e.g.
/// `emit_io_intrinsic` returning to `translate_body`'s Call
/// arm). Without the post-exit block switch, the CLIF
/// verifier panics in `FunctionBuilder::ins()` with "you
/// cannot add an instruction to a block already filled" on
/// the next insn.
pub(crate) fn trap_and_resume(
  tctx: &mut TCtx<'_>,
  builder: &mut FunctionBuilder,
  ctx: &mut FunCtx,
) {
  emit_exit_1(tctx, builder);

  ctx.terminated = true;

  let dead = builder.create_block();

  builder.switch_to_block(dead);

  ctx.terminated = false;
}
