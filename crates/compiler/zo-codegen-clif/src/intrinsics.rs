//! zo intrinsic emission: `show` / `showln` / `eshow` /
//! `eshowln` / `check`.
//!
//! Matches the ARM backend's open-coded behavior — each `show`
//! variant dispatches on the argument's zo `TyId` (tracked per-
//! function in `ctx.value_types`) and emits the appropriate
//! libc primitive. `check` lowers to a `brif` + `exit(1)` on
//! the fail branch.

use crate::context::{FunCtx, TCtx};
use crate::runtime::{
  emit_exit_1, emit_write_call, ensure_anon_data, ensure_libc_func,
  trap_and_resume,
};
use crate::types::is_unsigned_int;

use zo_token::Base;
use zo_ty::TyId;
use zo_value::ValueId;

use cranelift::codegen::ir;
use cranelift::codegen::ir::condcodes::IntCC;
use cranelift::codegen::ir::{
  AbiParam, InstBuilder, MemFlags, StackSlotData, StackSlotKind,
};
use cranelift::frontend::FunctionBuilder;
use cranelift_module::Module;

/// Formats an integer into a 32-byte stack buffer via libc
/// `snprintf("%lld", ...)` and pipes the result to `write(fd,
/// buf, len)`. Widens narrower SIR int types to I64 first —
/// unsigned via `uextend`, signed via `sextend` — so a single
/// `%lld` format handles every integer `TyId` in 6..=14.
fn emit_int_show(
  tctx: &mut TCtx<'_>,
  builder: &mut FunctionBuilder,
  fd: i64,
  val: ir::Value,
  ty_id: TyId,
  base: Base,
) {
  let val_ty = builder.func.dfg.value_type(val);
  let val_i64 = if val_ty == ir::types::I64 {
    val
  } else if is_unsigned_int(ty_id) {
    builder.ins().uextend(ir::types::I64, val)
  } else {
    builder.ins().sextend(ir::types::I64, val)
  };

  let slot = builder.create_sized_stack_slot(StackSlotData::new(
    StackSlotKind::ExplicitSlot,
    32,
    0,
  ));
  let buf_ptr = builder.ins().stack_addr(tctx.ptr_ty, slot, 0);
  let buf_size = builder.ins().iconst(tctx.ptr_ty, 32);

  let len = if base == Base::Decimal {
    // Fast path, unchanged: snprintf("%lld", val).
    let fmt_id = ensure_anon_data(tctx, "fmt_int", b"%lld\0");
    let fmt_gv = tctx.module.declare_data_in_func(fmt_id, builder.func);
    let fmt_ptr = builder.ins().global_value(tctx.ptr_ty, fmt_gv);

    let snprintf_fid = ensure_libc_func(tctx, "snprintf", |ptr_ty, cc| {
      let mut sig = ir::Signature::new(cc);

      sig.params.push(AbiParam::new(ptr_ty)); // buf
      sig.params.push(AbiParam::new(ptr_ty)); // size (size_t)
      sig.params.push(AbiParam::new(ptr_ty)); // fmt
      sig.params.push(AbiParam::new(ir::types::I64)); // val
      sig.returns.push(AbiParam::new(ir::types::I32));

      sig
    });
    let snprintf_fref =
      tctx.module.declare_func_in_func(snprintf_fid, builder.func);
    let call = builder
      .ins()
      .call(snprintf_fref, &[buf_ptr, buf_size, fmt_ptr, val_i64]);
    let len_i32 = builder.inst_results(call)[0];

    builder.ins().uextend(tctx.ptr_ty, len_i32)
  } else {
    // `b#`/`o#`/`x#`: format in the literal's base via the
    // runtime helper (printf has no binary specifier). The
    // value is unchanged; only the printed digits differ.
    let radix = builder.ins().iconst(ir::types::I32, base.radix() as i64);

    let fid = ensure_libc_func(tctx, "zo_itoa_radix", |ptr_ty, cc| {
      let mut sig = ir::Signature::new(cc);

      sig.params.push(AbiParam::new(ptr_ty)); // buf
      sig.params.push(AbiParam::new(ptr_ty)); // size (size_t)
      sig.params.push(AbiParam::new(ir::types::I64)); // val
      sig.params.push(AbiParam::new(ir::types::I32)); // radix
      sig.returns.push(AbiParam::new(ir::types::I32));

      sig
    });
    let fref = tctx.module.declare_func_in_func(fid, builder.func);
    let call = builder
      .ins()
      .call(fref, &[buf_ptr, buf_size, val_i64, radix]);
    let len_i32 = builder.inst_results(call)[0];

    builder.ins().uextend(tctx.ptr_ty, len_i32)
  };

  emit_write_call(tctx, builder, fd, buf_ptr, len);
}

/// Formats an `f64` into a 32-byte stack buffer via the
/// `zo_ftoa_f64` runtime wrapper (compiled from
/// `zo-linker/src/runtime.c` and linked alongside the user's
/// object) and pipes the result to `write(fd, buf, len)`.
///
/// Why a wrapper instead of a direct `snprintf`: Cranelift's
/// `Module::declare_function` keys by external name and
/// rejects re-declarations with different signatures. The int
/// path already declared `snprintf` with an I64-tail signature
/// for GPR-based variadic dispatch; a second declaration for
/// the float path (F64 tail → XMM on SysV) would conflict. The
/// runtime wrapper exports `zo_ftoa_f64(buf, size, val: double)`
/// — a distinct name with a fixed non-variadic signature, so
/// CLIF can declare it independently.
///
/// `f32` is promoted to `f64` before the call to keep the
/// runtime API single-entry.
fn emit_float_show(
  tctx: &mut TCtx<'_>,
  builder: &mut FunctionBuilder,
  fd: i64,
  val: ir::Value,
) {
  let val_ty = builder.func.dfg.value_type(val);
  let val_f64 = if val_ty == ir::types::F64 {
    val
  } else {
    builder.ins().fpromote(ir::types::F64, val)
  };

  let slot = builder.create_sized_stack_slot(StackSlotData::new(
    StackSlotKind::ExplicitSlot,
    32,
    0,
  ));
  let buf_ptr = builder.ins().stack_addr(tctx.ptr_ty, slot, 0);
  let buf_size = builder.ins().iconst(tctx.ptr_ty, 32);

  let fid = ensure_libc_func(tctx, "zo_ftoa_f64", |ptr_ty, cc| {
    let mut sig = ir::Signature::new(cc);

    sig.params.push(AbiParam::new(ptr_ty)); // buf
    sig.params.push(AbiParam::new(ptr_ty)); // size (size_t)
    sig.params.push(AbiParam::new(ir::types::F64)); // val
    sig.returns.push(AbiParam::new(ir::types::I32));

    sig
  });
  let fref = tctx.module.declare_func_in_func(fid, builder.func);
  let call = builder.ins().call(fref, &[buf_ptr, buf_size, val_f64]);
  let len_i32 = builder.inst_results(call)[0];
  let len = builder.ins().uextend(tctx.ptr_ty, len_i32);

  emit_write_call(tctx, builder, fd, buf_ptr, len);
}

/// Emits a branchless bool show via two `select`s — no CFG
/// fork needed:
///
/// ```text
/// ptr = select(cond, "true",  "false")
/// len = select(cond,    4,       5   )
/// write(fd, ptr, len)
/// ```
fn emit_bool_show(
  tctx: &mut TCtx<'_>,
  builder: &mut FunctionBuilder,
  fd: i64,
  val: ir::Value,
) {
  let t_id = ensure_anon_data(tctx, "true", b"true");
  let f_id = ensure_anon_data(tctx, "false", b"false");

  let t_gv = tctx.module.declare_data_in_func(t_id, builder.func);
  let f_gv = tctx.module.declare_data_in_func(f_id, builder.func);
  let t_ptr = builder.ins().global_value(tctx.ptr_ty, t_gv);
  let f_ptr = builder.ins().global_value(tctx.ptr_ty, f_gv);

  let ptr = builder.ins().select(val, t_ptr, f_ptr);
  let four = builder.ins().iconst(tctx.ptr_ty, 4);
  let five = builder.ins().iconst(tctx.ptr_ty, 5);
  let len = builder.ins().select(val, four, five);

  emit_write_call(tctx, builder, fd, ptr, len);
}

/// UTF-8 encodes a Unicode scalar (UTF-32 in I32) into a
/// 4-byte stack buffer and calls `write(fd, buf, len)`.
///
/// Branchless: every possible first / second / third / fourth
/// byte is computed, and `select` chains pick the right value
/// per position based on the codepoint's range. Length is
/// likewise picked via select. Bytes past `len` are still
/// stored into the slot but the `write` call ignores them.
///
/// Range breakdown (matches RFC 3629):
/// - `cp < 0x80`:    `0xxxxxxx`                                      (1 byte)
/// - `cp < 0x800`:   `110xxxxx 10xxxxxx`                             (2 bytes)
/// - `cp < 0x10000`: `1110xxxx 10xxxxxx 10xxxxxx`                    (3 bytes)
/// - else:           `11110xxx 10xxxxxx 10xxxxxx 10xxxxxx`           (4 bytes)
fn emit_char_show(
  tctx: &mut TCtx<'_>,
  builder: &mut FunctionBuilder,
  fd: i64,
  val: ir::Value,
) {
  // Widen UTF-32 codepoint to I32 (it already is, but the
  // helper is robust to type drift) then to I64 for uniform
  // shift / mask arithmetic.
  let cp32 = {
    let ty = builder.func.dfg.value_type(val);

    if ty == ir::types::I32 {
      val
    } else {
      builder.ins().ireduce(ir::types::I32, val)
    }
  };

  // Range predicates.
  let lt_80 = builder.ins().icmp_imm(IntCC::UnsignedLessThan, cp32, 0x80);
  let lt_800 = builder.ins().icmp_imm(IntCC::UnsignedLessThan, cp32, 0x800);
  let lt_10000 = builder
    .ins()
    .icmp_imm(IntCC::UnsignedLessThan, cp32, 0x10000);

  // Byte 0 — leading byte per range.
  let b0_ascii = builder.ins().band_imm(cp32, 0x7F);
  let b0_2 = {
    let shifted = builder.ins().ushr_imm(cp32, 6);
    let masked = builder.ins().band_imm(shifted, 0x1F);

    builder.ins().bor_imm(masked, 0xC0)
  };
  let b0_3 = {
    let shifted = builder.ins().ushr_imm(cp32, 12);
    let masked = builder.ins().band_imm(shifted, 0x0F);

    builder.ins().bor_imm(masked, 0xE0)
  };
  let b0_4 = {
    let shifted = builder.ins().ushr_imm(cp32, 18);
    let masked = builder.ins().band_imm(shifted, 0x07);

    builder.ins().bor_imm(masked, 0xF0)
  };

  // Byte 1 — first continuation byte for lengths ≥ 2.
  let cont_cp_raw = builder.ins().band_imm(cp32, 0x3F);
  let cont_cp = builder.ins().bor_imm(cont_cp_raw, 0x80);
  let cont_shift6 = {
    let s = builder.ins().ushr_imm(cp32, 6);
    let m = builder.ins().band_imm(s, 0x3F);

    builder.ins().bor_imm(m, 0x80)
  };
  let cont_shift12 = {
    let s = builder.ins().ushr_imm(cp32, 12);
    let m = builder.ins().band_imm(s, 0x3F);

    builder.ins().bor_imm(m, 0x80)
  };

  let b1_for_2 = cont_cp;
  let b1_for_3 = cont_shift6;
  let b1_for_4 = cont_shift12;

  // Byte 2 — only meaningful for lengths ≥ 3.
  let b2_for_3 = cont_cp;
  let b2_for_4 = cont_shift6;

  // Byte 3 — only meaningful for length = 4.
  let b3_for_4 = cont_cp;

  let zero = builder.ins().iconst(ir::types::I32, 0);

  // select chain per byte position: pick the value for the
  // matching range, zero otherwise. Built bottom-up so each
  // intermediate result lives in its own let-binding —
  // keeps only one mutable borrow of `builder` active at a
  // time.
  let b0_mid = builder.ins().select(lt_10000, b0_3, b0_4);
  let b0_inner = builder.ins().select(lt_800, b0_2, b0_mid);
  let b0 = builder.ins().select(lt_80, b0_ascii, b0_inner);

  let b1_mid = builder.ins().select(lt_10000, b1_for_3, b1_for_4);
  let b1_inner = builder.ins().select(lt_800, b1_for_2, b1_mid);
  let b1 = builder.ins().select(lt_80, zero, b1_inner);

  let b2_inner = builder.ins().select(lt_10000, b2_for_3, b2_for_4);
  let b2 = builder.ins().select(lt_800, zero, b2_inner);

  let b3 = builder.ins().select(lt_10000, zero, b3_for_4);

  // Length per range.
  let one = builder.ins().iconst(tctx.ptr_ty, 1);
  let two = builder.ins().iconst(tctx.ptr_ty, 2);
  let three = builder.ins().iconst(tctx.ptr_ty, 3);
  let four = builder.ins().iconst(tctx.ptr_ty, 4);
  let len_mid = builder.ins().select(lt_10000, three, four);
  let len_inner = builder.ins().select(lt_800, two, len_mid);
  let len = builder.ins().select(lt_80, one, len_inner);

  // Truncate each byte to I8 and stash into the stack slot.
  let slot = builder.create_sized_stack_slot(StackSlotData::new(
    StackSlotKind::ExplicitSlot,
    4,
    0,
  ));
  let buf_ptr = builder.ins().stack_addr(tctx.ptr_ty, slot, 0);

  for (i, b) in [b0, b1, b2, b3].iter().enumerate() {
    let b8 = builder.ins().ireduce(ir::types::I8, *b);

    builder.ins().stack_store(b8, slot, i as i32);
  }

  emit_write_call(tctx, builder, fd, buf_ptr, len);
}

/// Translates `check(cond: bool)` into a branch: on `cond ==
/// true` the program continues; on `cond == false` the
/// program exits with code 1. zo's source `check@eq(a, b)`
/// desugars to `check(a == b)` at SIR time, so this one arm
/// covers every `check@op` variant.
pub(crate) fn emit_check_intrinsic(
  tctx: &mut TCtx<'_>,
  builder: &mut FunctionBuilder,
  ctx: &mut FunCtx,
  dst: ValueId,
  args: &[ValueId],
) {
  // Missing arg → abort anyway (defensive; semantic analyzer
  // shouldn't let this through).
  let Some(&arg_id) = args.first() else {
    trap_and_resume(tctx, builder, ctx);

    return;
  };

  let Some(cond) = ctx.values.get(&arg_id).copied() else {
    trap_and_resume(tctx, builder, ctx);

    return;
  };

  let pass_block = builder.create_block();
  let fail_block = builder.create_block();

  builder.ins().brif(cond, pass_block, &[], fail_block, &[]);

  // Fail branch — `exit(1)` terminates the block.
  builder.switch_to_block(fail_block);
  emit_exit_1(tctx, builder);

  // Pass branch — continuation.
  builder.switch_to_block(pass_block);

  ctx.terminated = false;

  // `check` returns unit; sentinel keeps downstream references
  // to `dst` consistent with every other unit-producing arm.
  let sentinel = builder.ins().iconst(ir::types::I8, 0);

  ctx.values.insert(dst, sentinel);
}

/// Translates `show` / `showln` / `eshow` / `eshowln` into
/// direct libc calls, matching the ARM backend's open-coded
/// behavior without needing a separate runtime crate.
/// Dispatches on the argument's zo `TyId` (tracked per-
/// function in `ctx.value_types`):
///
/// - **`str` / `bytes`** (TyId 4 / 5): decompose the
///   `[u64 LE len, bytes]` header and `write(fd, data, len)`.
/// - **bool** (TyId 2): branchless `select` on pre-allocated
///   `"true"` / `"false"` data objects + single `write`.
/// - **char** (TyId 3): inline UTF-8 encoder into a 4-byte
///   stack buffer + single `write`.
/// - **integers** (TyId 6..=14): widen to I64 and route
///   through libc `snprintf` + `write`.
/// - **floats** (TyId 15..=17): promote to F64 and route
///   through the `zo_ftoa_f64` runtime wrapper + `write`.
/// - **anything else** (aggregates): trap — recursive
///   formatting of tuples / structs / arrays isn't wired.
///
/// `fd = 1` for `show` / `showln` (stdout), `fd = 2` for
/// `eshow` / `eshowln` (stderr). `showln` / `eshowln` emit
/// a trailing `"\n"` write regardless of the dispatched arm.
pub(crate) fn emit_io_intrinsic(
  tctx: &mut TCtx<'_>,
  builder: &mut FunctionBuilder,
  ctx: &mut FunCtx,
  dst: ValueId,
  name: &str,
  args: &[ValueId],
) {
  let fd: i64 = if name.starts_with('e') { 2 } else { 1 };
  let with_newline = name.ends_with("ln");

  let Some(&arg_id) = args.first() else {
    // No arg — unit sentinel, no I/O.
    let sentinel = builder.ins().iconst(ir::types::I8, 0);

    ctx.values.insert(dst, sentinel);

    return;
  };

  let Some(arg_val) = ctx.values.get(&arg_id).copied() else {
    trap_and_resume(tctx, builder, ctx);

    return;
  };

  let arg_ty_id = ctx.value_types.get(&arg_id).copied().unwrap_or(TyId(0));

  match arg_ty_id.0 {
    // Str — pointer to `[u64 LE len, utf-8 bytes]` header.
    4 => {
      let len = builder.ins().load(tctx.ptr_ty, MemFlags::new(), arg_val, 0);
      let data_ptr = builder.ins().iadd_imm(arg_val, 8);

      emit_write_call(tctx, builder, fd, data_ptr, len);
    }
    // Integers (signed + unsigned) and byte scalars. zo's
    // `bytes` (TyId 5) is a family — a scalar byte literal
    // (`` `z` ``) carries the byte value directly, while a
    // `[]byte` slice would carry a header pointer. The test
    // suite only exercises the scalar form with `show`, so we
    // route the whole TyId through the int formatter;
    // slice-with-show needs a type-table discriminator.
    5..=14 => {
      // SPARSE: a missing entry is plain decimal, so this never
      // affects existing `showln(int)`.
      let base = tctx
        .int_bases
        .get(&arg_id.0)
        .copied()
        .unwrap_or(Base::Decimal);

      emit_int_show(tctx, builder, fd, arg_val, arg_ty_id, base);
    }
    // Bool.
    2 => {
      emit_bool_show(tctx, builder, fd, arg_val);
    }
    // Char (UTF-32).
    3 => {
      emit_char_show(tctx, builder, fd, arg_val);
    }
    // Floats (f32, f64, arch-float).
    15..=17 => {
      emit_float_show(tctx, builder, fd, arg_val);
    }
    // Aggregates (tuple / struct / array / enum) fall here —
    // they need recursive per-field formatting that isn't
    // wired yet. Abort cleanly so the user sees a clear exit
    // code instead of garbage output.
    _ => {
      trap_and_resume(tctx, builder, ctx);

      return;
    }
  }

  if with_newline {
    let nl_id = ensure_anon_data(tctx, "newline", b"\n");
    let nl_gv = tctx.module.declare_data_in_func(nl_id, builder.func);
    let nl_ptr = builder.ins().global_value(tctx.ptr_ty, nl_gv);
    let one = builder.ins().iconst(tctx.ptr_ty, 1);

    emit_write_call(tctx, builder, fd, nl_ptr, one);
  }

  // `show` family returns unit. Materialize an `I8` sentinel
  // so any downstream refs to `dst` don't miss `ctx.values`.
  let sentinel = builder.ins().iconst(ir::types::I8, 0);

  ctx.values.insert(dst, sentinel);
}
