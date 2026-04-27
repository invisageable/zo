//! SIR → CLIF instruction translator.
//!
//! Per-module pipeline:
//!   1. Scan for module-scope `val` bindings
//!      (`collect_const_defs`) so `Load { Local(NAME) }` can
//!      inline the literal at every use site.
//!   2. Declaration sweep — `declare_function` for every
//!      `Insn::FunDef` (imports for empty-body functions,
//!      exports otherwise) so forward calls resolve.
//!   3. Per-body: determine range
//!      `[body_start .. next_fundef_or_end]`, pre-allocate a
//!      CLIF `Block` for every `Insn::Label` so forward jumps
//!      can reference them, then walk the insns.
//!
//! Module-level markers that the executor interleaves with
//! real work (`PackDecl`, `ModuleLoad`, type definitions,
//! `Directive`) are explicit no-ops at codegen time.

use crate::context::{
  AGG_ALIGN_SHIFT, AGG_SLOT_SIZE, ConstLiteral, FunCtx, TCtx,
};
use crate::intrinsics::{emit_check_intrinsic, emit_io_intrinsic};
use crate::runtime::{emit_exit_1, ensure_libc_func};
use crate::types::{is_float, is_unsigned_int, pointer_ty, ty_id_to_clif};

use zo_interner::{Interner, Symbol};
use zo_sir::{BinOp, Insn, LoadSource, UnOp};
use zo_ty::TyId;
use zo_value::ValueId;

use cranelift::codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift::codegen::ir::{
  AbiParam, Function, InstBuilder, MemFlags, StackSlotData, StackSlotKind,
  UserFuncName,
};
use cranelift::codegen::isa::CallConv;
use cranelift::codegen::{Context, ir};
use cranelift::frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_module::{DataDescription, DataId, FuncId, Linkage, Module};
use cranelift_object::ObjectModule;
use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};

/// Translates the whole SIR instruction stream into the given
/// [`ObjectModule`]. One CLIF function per SIR `FunDef`.
///
/// Returns the concatenated CLIF IR text of every defined
/// function (via cranelift's `ir::Function` `Display` impl).
/// Callers that only care about object bytes can ignore it;
/// `CliftGen::generate_asm` consumes it for the `--emit asm`
/// debug view. Formatting is per-function and runs right
/// before `define_function` consumes the `Context`.
pub(crate) fn translate_module(
  module: &mut ObjectModule,
  interner: &Interner,
  insns: &[Insn],
) -> String {
  let call_conv = module.target_config().default_call_conv;
  let ptr_ty = pointer_ty(module);

  // First pass: declare every function so forward calls
  // inside bodies can resolve their `FuncId`.
  let mut func_ids: HashMap<Symbol, FuncId> = HashMap::default();
  // Persistent across the whole module so repeat `ConstString`s
  // for the same `Symbol` share one data object.
  let mut const_strings: HashMap<Symbol, DataId> = HashMap::default();
  // Guards against the second pass calling `define_function`
  // twice for one `FuncId`. Happens when the same symbol
  // (e.g. `gcd`) is emitted as a FunDef in both the user
  // program and zo's stdlib — `declare_function` merges
  // these into one `FuncId`, but each still shows up as an
  // `Insn::FunDef` in the body-definition loop.
  let mut defined_funcs: HashSet<FuncId> = HashSet::default();
  // Module-scope `val NAME = lit;` bindings resolved to their
  // raw literal so every `Load { Local(NAME) }` can inline.
  let const_defs = collect_const_defs(insns);
  // Lazily populated by the I/O intercept. Kept at module
  // scope so every function body shares one `FuncId` per
  // libc symbol and one `DataId` per reusable blob.
  let mut libc_funcs: HashMap<&'static str, FuncId> = HashMap::default();
  let mut anon_data: HashMap<&'static str, DataId> = HashMap::default();

  for (idx, insn) in insns.iter().enumerate() {
    if let Insn::FunDef {
      name,
      params,
      return_ty,
      body_start,
      ..
    } = insn
    {
      // Linkage is driven by body presence, not by
      // `FunctionKind` — the executor stamps `Intrinsic` on
      // every function whose SIR body is empty, which catches
      // both real FFI imports (`show`, `showln`) AND user
      // stubs (`fun main() {}`). An empty-body function maps
      // to `Linkage::Import` (stays an unresolved symbol that
      // the system linker fills in); a body-bearing function
      // maps to `Linkage::Export` (this module owns the
      // definition).
      let linkage = if fundef_body_is_empty(insns, idx, *body_start) {
        Linkage::Import
      } else {
        Linkage::Export
      };

      // Cranelift's `ObjectModule` handles platform-specific
      // symbol mangling (e.g. leading `_` for Mach-O) — pass
      // the raw name.
      let fname = interner.get(*name);
      let is_main = fname == "main";
      let sig = build_signature(params, *return_ty, call_conv, ptr_ty, is_main);

      let func_id = module
        .declare_function(fname, linkage, &sig)
        .expect("declare_function failed");

      func_ids.insert(*name, func_id);
    }
  }

  // Accumulator for CLIF IR text — consumed by the `--emit
  // asm` path on CliftGen. Zero cost when discarded.
  let mut ir_text = String::new();

  // Second pass: define each non-intrinsic function's body.
  let mut i = 0;

  while i < insns.len() {
    let Insn::FunDef {
      name,
      params,
      return_ty,
      body_start,
      ..
    } = &insns[i]
    else {
      i += 1;
      continue;
    };

    // Body range for this function. `body_start = 0` is the
    // executor's sentinel for "no body"; clamp to `i + 1` so
    // the scan starts after the current FunDef instead of
    // re-finding it.
    let body_start_u = (*body_start as usize).max(i + 1);
    let end = next_fundef_after(insns, i + 1).unwrap_or(insns.len());

    // Empty body → declared as `Linkage::Import` in the first
    // pass. Nothing to define; skip to the next function.
    if body_start_u >= end {
      i = end;
      continue;
    }

    let func_id = func_ids[name];

    // Skip duplicate definitions. `declare_function` merges
    // multiple declarations of the same name into one FuncId,
    // but the second-pass body loop would still call
    // `define_function` for each FunDef with that name —
    // Cranelift errors with `DuplicateDefinition` on the
    // second call. Common trigger: the user defines a
    // function whose name already lives in zo's stdlib.
    if !defined_funcs.insert(func_id) {
      i = end;
      continue;
    }

    let is_main = interner.get(*name) == "main";
    let sig = build_signature(params, *return_ty, call_conv, ptr_ty, is_main);

    let mut ctx = Context::new();

    ctx.func = Function::with_name_signature(
      UserFuncName::user(0, func_id.as_u32()),
      sig,
    );

    let mut fbctx = FunctionBuilderContext::new();
    let mut builder = FunctionBuilder::new(&mut ctx.func, &mut fbctx);

    let entry = builder.create_block();

    builder.append_block_params_for_function_params(entry);
    builder.switch_to_block(entry);
    builder.seal_block(entry);

    let mut fun_ctx = FunCtx::new(is_main);

    // Seed `Variable`s from the entry block's parameters.
    // Every param is pushed into `params` (for index-keyed
    // `Load { Param(idx) }`) and mirrored into `vars` under
    // its declaration name (for name-keyed `Store { name }`
    // if a mutable param is later reassigned).
    for (idx, (sym, pty)) in params.iter().enumerate() {
      let ty = ty_id_to_clif(*pty, ptr_ty);
      let v = builder.block_params(entry)[idx];
      let var = fun_ctx.declare_local(&mut builder, *sym, ty);

      builder.def_var(var, v);
      fun_ctx.params.push(var);
    }

    // Label pre-pass: allocate a CLIF block per SIR
    // `Label { id }` in the body so forward jumps / brifs can
    // reference them before we walk past. Without this, a
    // `Jump { target: 0 }` emitted at SIR index 5 can't
    // reference the block for `Label { id: 0 }` at index 12.
    preallocate_label_blocks(
      &mut builder,
      &mut fun_ctx,
      &insns[body_start_u..end],
    );

    // Inner scope so `TCtx`'s borrow of `module` /
    // `const_strings` ends before `module.define_function`
    // below — `define_function` needs `&mut module` which
    // would otherwise conflict.
    {
      let mut tctx = TCtx {
        module,
        interner,
        func_ids: &func_ids,
        const_strings: &mut const_strings,
        const_defs: &const_defs,
        libc_funcs: &mut libc_funcs,
        anon_data: &mut anon_data,
        ptr_ty,
      };

      translate_body(
        &mut tctx,
        &mut builder,
        &mut fun_ctx,
        &insns[body_start_u..end],
      );

      // Guarantee every block has a terminator before sealing.
      // The `Return` / `Jump` arms always create a trailing
      // dead block after their terminator so subsequent
      // insns don't panic the builder — if that dead block
      // turns out to be the LAST block of the function, it's
      // left empty.
      //
      // Also: lazy-created blocks for Jump targets whose
      // `Label` was never emitted (see the Jump arm) are
      // reachable but orphan — their entry jumps in, nothing
      // else exits. Switching to each and emitting
      // `exit(1)` gives them a valid terminator; Cranelift
      // DCEs any that turn out to be unreachable.
      if !fun_ctx.terminated {
        emit_exit_1(&mut tctx, &mut builder);
      }

      let orphan_blocks: Vec<ir::Block> =
        fun_ctx.blocks.values().copied().collect();

      for block in orphan_blocks {
        // Already terminated? Skip. `last_inst` is `None` for
        // empty blocks (needs a terminator) or the block's
        // last emitted insn (check its opcode).
        let has_terminator =
          builder.func.layout.last_inst(block).is_some_and(|inst| {
            builder.func.dfg.insts[inst].opcode().is_terminator()
          });

        if has_terminator {
          continue;
        }

        builder.switch_to_block(block);

        emit_exit_1(&mut tctx, &mut builder);
      }
    }

    // Seal every block — tells cranelift all predecessors are
    // now known, finalizes any pending SSA phi nodes.
    builder.seal_all_blocks();
    builder.finalize();

    // Capture the formatted function before `define_function`
    // consumes the context. Cranelift's `ir::Function` has a
    // Display impl that writes CLIF text directly.
    use std::fmt::Write as _;
    let _ = writeln!(ir_text, "{}", ctx.func);

    module
      .define_function(func_id, &mut ctx)
      .expect("define_function failed");

    i = end;
  }

  ir_text
}

/// Returns `true` if the FunDef at `fundef_idx` has no body
/// insns in the SIR stream. A body is "empty" when the slice
/// `[body_start_u .. next_fundef_or_end]` is empty, where
/// `body_start_u = max(body_start, fundef_idx + 1)`. Real FFI
/// intrinsics carry `body_start = 0` (sentinel) and are
/// immediately followed by the next FunDef, so their slice is
/// empty. User stubs like `fun main() {}` have a real
/// `body_start` but only an implicit `Return`, which the
/// executor flattens into an empty slice when no other
/// instructions are emitted — those also land here when they
/// happen to end the stream.
fn fundef_body_is_empty(
  insns: &[Insn],
  fundef_idx: usize,
  body_start: u32,
) -> bool {
  let body_start_u = (body_start as usize).max(fundef_idx + 1);
  let end = next_fundef_after(insns, fundef_idx + 1).unwrap_or(insns.len());

  body_start_u >= end
}

/// Returns the index of the next `FunDef` after `start`, or
/// `None` if this is the last function in the stream.
fn next_fundef_after(insns: &[Insn], start: usize) -> Option<usize> {
  insns[start..]
    .iter()
    .position(|i| matches!(i, Insn::FunDef { .. }))
    .map(|off| start + off)
}

/// Pre-allocates a CLIF `Block` for every `Insn::Label { id }`
/// in the body. CLIF requires every block referenced by a
/// `jump` / `brif` to already exist at emit time; SIR's forward
/// jumps (loop-head labels, if-else merge points) mean we
/// can't wait until the `Label` insn itself to create its
/// block. Two-pass: allocate first, fill bodies second.
fn preallocate_label_blocks(
  builder: &mut FunctionBuilder,
  ctx: &mut FunCtx,
  body: &[Insn],
) {
  for insn in body {
    if let Insn::Label { id } = insn {
      let block = builder.create_block();

      ctx.blocks.insert(*id, block);
    }
  }
}

/// First-pass scan: walk the SIR stream, locate every
/// `Insn::ConstDef { name, value, .. }`, and resolve its
/// `value: ValueId` back to the `Const*` insn that produced it.
/// Returns a map so `Load { Local(NAME) }` can inline the
/// literal at every use.
///
/// Conservative: if a ConstDef's producer isn't a direct
/// literal (e.g. computed expression SIR wasn't pre-folded),
/// the name is simply absent from the map — Load falls through
/// to the normal `vars` path and either resolves there or
/// traps. Matches plan row 6's "inlined at uses" intent
/// without depending on a pre-fold guarantee.
fn collect_const_defs(insns: &[Insn]) -> HashMap<Symbol, ConstLiteral> {
  let mut out: HashMap<Symbol, ConstLiteral> = HashMap::default();

  for insn in insns {
    let Insn::ConstDef { name, value, .. } = insn else {
      continue;
    };

    let Some(lit) = insns.iter().find_map(|producer| match producer {
      Insn::ConstInt {
        dst,
        value: v,
        ty_id,
      } if dst == value => Some(ConstLiteral::Int {
        value: *v,
        ty_id: *ty_id,
      }),
      Insn::ConstFloat {
        dst,
        value: v,
        ty_id,
      } if dst == value => Some(ConstLiteral::Float {
        value: *v,
        ty_id: *ty_id,
      }),
      Insn::ConstBool { dst, value: v, .. } if dst == value => {
        Some(ConstLiteral::Bool { value: *v })
      }
      Insn::ConstString { dst, symbol, .. } if dst == value => {
        Some(ConstLiteral::Str { symbol: *symbol })
      }
      _ => None,
    }) else {
      continue;
    };

    out.insert(*name, lit);
  }

  out
}

/// Materializes a [`ConstLiteral`] at a use site. Mirrors the
/// same CLIF emission used for fresh literal insns so the
/// inlined form is indistinguishable from a direct literal.
fn materialize_const_literal(
  tctx: &mut TCtx<'_>,
  builder: &mut FunctionBuilder,
  lit: ConstLiteral,
) -> ir::Value {
  match lit {
    ConstLiteral::Int { value, ty_id } => {
      let ty = ty_id_to_clif(ty_id, ir::types::I64);

      builder.ins().iconst(ty, value as i64)
    }
    ConstLiteral::Float { value, ty_id } => {
      if ty_id.0 == 15 {
        builder.ins().f32const(value as f32)
      } else {
        builder.ins().f64const(value)
      }
    }
    ConstLiteral::Bool { value } => {
      builder.ins().iconst(ir::types::I8, i64::from(value))
    }
    ConstLiteral::Str { symbol } => {
      // Mirrors the `Insn::ConstString` arm — re-uses the
      // per-symbol dedup map so multiple Loads of the same
      // ConstDef point at a single data object.
      let data_id = if let Some(id) = tctx.const_strings.get(&symbol).copied() {
        id
      } else {
        let s = tctx.interner.get(symbol);
        let bytes = s.as_bytes();
        let len = bytes.len() as u64;

        let mut buf: Vec<u8> = Vec::with_capacity(8 + bytes.len());

        buf.extend_from_slice(&len.to_le_bytes());
        buf.extend_from_slice(bytes);

        let mut desc = DataDescription::new();

        desc.define(buf.into_boxed_slice());

        let id = tctx
          .module
          .declare_anonymous_data(false, false)
          .expect("declare_anonymous_data failed");

        tctx
          .module
          .define_data(id, &desc)
          .expect("define_data failed");

        tctx.const_strings.insert(symbol, id);

        id
      };

      let gv = tctx.module.declare_data_in_func(data_id, builder.func);

      builder.ins().global_value(tctx.ptr_ty, gv)
    }
  }
}

/// Translates a SIR `Cast { src, from_ty, to_ty }` — emits the
/// right CLIF conversion op based on the from/to type
/// categories. Four cases:
/// - **int → int**: `uextend` / `sextend` / `ireduce` depending
///   on direction and signedness.
/// - **float → float**: `fpromote` / `fdemote`.
/// - **int → float**: `fcvt_from_uint` / `fcvt_from_sint`.
/// - **float → int**: `fcvt_to_uint_sat` / `fcvt_to_sint_sat`
///   — the saturating variants avoid UB on out-of-range inputs.
///
/// Same-width same-category casts are a no-op (identity).
fn translate_cast(
  builder: &mut FunctionBuilder,
  src: ir::Value,
  from_ty: TyId,
  to_ty: TyId,
  ptr_ty: ir::Type,
) -> ir::Value {
  let from_clif = ty_id_to_clif(from_ty, ptr_ty);
  let to_clif = ty_id_to_clif(to_ty, ptr_ty);

  if from_clif == to_clif {
    return src;
  }

  let from_is_float = is_float(from_ty);
  let to_is_float = is_float(to_ty);

  match (from_is_float, to_is_float) {
    (false, false) => {
      // int → int.
      if from_clif.bits() < to_clif.bits() {
        if is_unsigned_int(from_ty) {
          builder.ins().uextend(to_clif, src)
        } else {
          builder.ins().sextend(to_clif, src)
        }
      } else {
        builder.ins().ireduce(to_clif, src)
      }
    }
    (true, true) => {
      // float → float.
      if from_clif.bits() < to_clif.bits() {
        builder.ins().fpromote(to_clif, src)
      } else {
        builder.ins().fdemote(to_clif, src)
      }
    }
    (false, true) => {
      // int → float.
      if is_unsigned_int(from_ty) {
        builder.ins().fcvt_from_uint(to_clif, src)
      } else {
        builder.ins().fcvt_from_sint(to_clif, src)
      }
    }
    (true, false) => {
      // float → int (saturating — NaN / out-of-range map to a
      // defined value instead of UB).
      if is_unsigned_int(to_ty) {
        builder.ins().fcvt_to_uint_sat(to_clif, src)
      } else {
        builder.ins().fcvt_to_sint_sat(to_clif, src)
      }
    }
  }
}

/// Records `(dst ValueId → ty_id)` for every value-producing
/// insn so `emit_io_intrinsic` can dispatch `show` / `showln`
/// by argument type. Called as a pre-pass inside
/// `translate_body`'s main loop, BEFORE the insn is handled —
/// that way the mapping is visible if the same body later
/// calls `show` against this value.
fn record_value_type(ctx: &mut FunCtx, insn: &Insn) {
  match insn {
    Insn::ConstInt { dst, ty_id, .. }
    | Insn::ConstFloat { dst, ty_id, .. }
    | Insn::ConstBool { dst, ty_id, .. }
    | Insn::ConstString { dst, ty_id, .. }
    | Insn::Load { dst, ty_id, .. }
    | Insn::Call { dst, ty_id, .. }
    | Insn::BinOp { dst, ty_id, .. }
    | Insn::UnOp { dst, ty_id, .. }
    | Insn::ArrayLiteral { dst, ty_id, .. }
    | Insn::ArrayIndex { dst, ty_id, .. }
    | Insn::ArrayLen { dst, ty_id, .. }
    | Insn::ArrayPop { dst, ty_id, .. }
    | Insn::TupleLiteral { dst, ty_id, .. }
    | Insn::TupleIndex { dst, ty_id, .. }
    | Insn::StructConstruct { dst, ty_id, .. }
    | Insn::EnumConstruct { dst, ty_id, .. } => {
      ctx.value_types.insert(*dst, *ty_id);
    }
    Insn::Cast { dst, to_ty, .. } => {
      ctx.value_types.insert(*dst, *to_ty);
    }
    _ => {}
  }
}

/// Allocates a no-length-prefix aggregate (tuple or struct),
/// stores each field at offset `i * 8`, and returns the
/// slot's address. Returns `None` on any missing field value
/// after emitting a trap + marking `ctx.terminated` — the
/// caller's match arm must return early in that case.
///
/// Shared between `Insn::TupleLiteral` and
/// `Insn::StructConstruct` — same `Vec<ValueId>` shape, same
/// uniform 8-byte-slot layout. Arrays use the length-prefix
/// variant directly in their own arm.
fn emit_aggregate_literal(
  tctx: &mut TCtx<'_>,
  builder: &mut FunctionBuilder,
  ctx: &mut FunCtx,
  elements: &[ValueId],
) -> Option<ir::Value> {
  let slot = builder.create_sized_stack_slot(StackSlotData::new(
    StackSlotKind::ExplicitSlot,
    (elements.len() as u32) * AGG_SLOT_SIZE,
    AGG_ALIGN_SHIFT,
  ));

  for (i, eid) in elements.iter().enumerate() {
    let Some(v) = ctx.values.get(eid).copied() else {
      emit_exit_1(tctx, builder);

      ctx.terminated = true;

      return None;
    };

    let offset = ((i as u32) * AGG_SLOT_SIZE) as i32;

    builder.ins().stack_store(v, slot, offset);
  }

  Some(builder.ins().stack_addr(tctx.ptr_ty, slot, 0))
}

/// Widens `idx` to pointer width via `uextend` if it's
/// narrower; returns unchanged if already pointer-wide. Used by
/// `ArrayIndex` / `ArrayStore` before computing `base + idx*8`
/// — the addition needs both operands at ptr width.
fn widen_to_ptr(
  builder: &mut FunctionBuilder,
  idx: ir::Value,
  ptr_ty: ir::Type,
) -> ir::Value {
  let idx_ty = builder.func.dfg.value_type(idx);

  if idx_ty == ptr_ty {
    idx
  } else {
    builder.ins().uextend(ptr_ty, idx)
  }
}

/// Ensures the current block has a terminator before switching
/// away. CLIF forbids a non-terminated block — zo's hand-
/// written ARM codegen relies on implicit fall-through, so we
/// synthesize a `jump` whenever the mirrored `terminated` flag
/// says the builder still needs one.
fn seal_current_with_jump(
  builder: &mut FunctionBuilder,
  ctx: &mut FunCtx,
  next: ir::Block,
) {
  if !ctx.terminated {
    builder.ins().jump(next, &[]);
  }

  ctx.terminated = false;
}

/// Builds a CLIF [`ir::Signature`] from SIR param / return
/// types. zo's unit type (`TyId(1)`) is omitted from the
/// returns vec so void functions produce `ret void`.
fn build_signature(
  params: &[(Symbol, TyId)],
  return_ty: TyId,
  call_conv: CallConv,
  ptr_ty: ir::Type,
  is_main: bool,
) -> ir::Signature {
  let mut sig = ir::Signature::new(call_conv);

  for (_, pty) in params {
    sig.params.push(AbiParam::new(ty_id_to_clif(*pty, ptr_ty)));
  }

  if is_main {
    // C's `crt0` / `crt1` entry stub calls `main` and reads
    // its return register as the process exit code. zo's
    // `fun main()` has return type `unit` (TyId(1)), which
    // would normally produce a zero-return signature — the
    // exit register would be uninitialized garbage. Force
    // the CLIF signature to `() -> i32` so the `Return`
    // handler can inject `iconst(I32, 0)` on implicit
    // returns (and pass through any explicit int return
    // from future `fun main(): int { ... }` programs).
    sig.returns.push(AbiParam::new(ir::types::I32));
  } else if return_ty != TyId(1) {
    sig
      .returns
      .push(AbiParam::new(ty_id_to_clif(return_ty, ptr_ty)));
  }

  sig
}

/// Walks the body instructions and emits CLIF via the given
/// [`FunctionBuilder`]. [`TCtx`] carries module-wide state
/// needed by insns that touch globals (`ConstString` allocates
/// data; `Call` imports the callee's `FuncId`).
fn translate_body(
  tctx: &mut TCtx<'_>,
  builder: &mut FunctionBuilder,
  ctx: &mut FunCtx,
  body: &[Insn],
) {
  for insn in body {
    // Track the (dst, ty_id) mapping for every
    // value-producing insn so `emit_io_intrinsic` can
    // dispatch `show` / `showln` by argument type. Mirrors
    // `zo-codegen-arm`'s `value_types` pre-pass
    // (`codegen.rs:960`).
    record_value_type(ctx, insn);

    match insn {
      Insn::ConstInt { dst, value, ty_id } => {
        let ty = ty_id_to_clif(*ty_id, ir::types::I64);
        let v = builder.ins().iconst(ty, *value as i64);

        ctx.values.insert(*dst, v);
      }
      Insn::ConstFloat { dst, value, ty_id } => {
        let v = if ty_id.0 == 15 {
          // f32.
          builder.ins().f32const(*value as f32)
        } else {
          // f64 / arch.
          builder.ins().f64const(*value)
        };

        ctx.values.insert(*dst, v);
      }
      Insn::ConstBool { dst, value, .. } => {
        let v = builder.ins().iconst(ir::types::I8, i64::from(*value));

        ctx.values.insert(*dst, v);
      }
      Insn::ConstString { dst, symbol, .. } => {
        // Layout: one module-level data object per distinct
        // string, shaped `[len: u64 LE, ...utf-8 bytes]`. The
        // dst receives the data object's address.
        //
        // Little-endian is hard-coded — every Cranelift target
        // we cover (x86_64, aarch64) is LE. Re-evaluate if a
        // BE target is ever added.
        let data_id = if let Some(id) = tctx.const_strings.get(symbol).copied()
        {
          id
        } else {
          let s = tctx.interner.get(*symbol);
          let bytes = s.as_bytes();
          let len = bytes.len() as u64;

          let mut buf: Vec<u8> = Vec::with_capacity(8 + bytes.len());

          buf.extend_from_slice(&len.to_le_bytes());
          buf.extend_from_slice(bytes);

          let mut desc = DataDescription::new();

          desc.define(buf.into_boxed_slice());

          let id = tctx
            .module
            .declare_anonymous_data(false, false)
            .expect("declare_anonymous_data failed");

          tctx
            .module
            .define_data(id, &desc)
            .expect("define_data failed");

          tctx.const_strings.insert(*symbol, id);

          id
        };

        let gv = tctx.module.declare_data_in_func(data_id, builder.func);
        let v = builder.ins().global_value(tctx.ptr_ty, gv);

        ctx.values.insert(*dst, v);
      }
      Insn::TupleLiteral { dst, elements, .. }
      | Insn::StructConstruct {
        dst,
        fields: elements,
        ..
      } => {
        // Shared tuple / struct layout: stack slot of `n * 8`,
        // field i stored at `i * 8`. No length prefix — arity
        // / field-count is compile-time-known at every
        // `TupleIndex` / `FieldStore` via the `index` field.
        //
        // Structs reuse the tuple shape: same `Vec<ValueId>`
        // fields, same compile-time offsets. `struct_name`
        // is ignored — layout is purely positional with the
        // 8-byte-uniform-slot convention.
        let Some(addr) = emit_aggregate_literal(tctx, builder, ctx, elements)
        else {
          return;
        };

        ctx.values.insert(*dst, addr);
      }
      Insn::EnumConstruct {
        dst,
        variant,
        fields,
        ..
      } => {
        // Enum layout (mirrors zo-codegen-arm
        // codegen.rs:2281): `[u64 LE tag, field_0, ...,
        // field_{n-1}]`. Tag at offset 0, field `i` at
        // `(i + 1) * 8`.
        //
        // Reads come through `TupleIndex`: index 0 → tag,
        // index `i + 1` → field `i`. No special read arm
        // needed — the tuple-access offsets serve enums
        // since the slot layout is positional.
        let slot_count = 1 + fields.len() as u32;
        let slot = builder.create_sized_stack_slot(StackSlotData::new(
          StackSlotKind::ExplicitSlot,
          slot_count * AGG_SLOT_SIZE,
          AGG_ALIGN_SHIFT,
        ));

        let tag = builder.ins().iconst(tctx.ptr_ty, *variant as i64);

        builder.ins().stack_store(tag, slot, 0);

        for (i, fid) in fields.iter().enumerate() {
          let Some(v) = ctx.values.get(fid).copied() else {
            emit_exit_1(tctx, builder);

            ctx.terminated = true;

            return;
          };

          let offset = (((i + 1) as u32) * AGG_SLOT_SIZE) as i32;

          builder.ins().stack_store(v, slot, offset);
        }

        let addr = builder.ins().stack_addr(tctx.ptr_ty, slot, 0);

        ctx.values.insert(*dst, addr);
      }
      Insn::ArrayLiteral { dst, elements, .. } => {
        // Length-prefixed array layout:
        // `[u64 LE len, elem_0, elem_1, ..., elem_{n-1}]`.
        // Stack slot is `8 + n * 8` bytes. `ArrayLen` loads
        // the prefix; `ArrayIndex` / `ArrayStore` shift the
        // data offset by +8.
        //
        // zo-codegen-arm keeps the length in a TyId-keyed
        // metadata table; diverging here avoids threading a
        // TyTable into the CLIF path just for `ArrayLen`, and
        // no zo program passes arrays between ARM- and CLIF-
        // compiled code so the layout divergence is self-
        // contained.
        let n = elements.len() as u64;
        let total = (AGG_SLOT_SIZE as u64) + n * (AGG_SLOT_SIZE as u64);
        let slot = builder.create_sized_stack_slot(StackSlotData::new(
          StackSlotKind::ExplicitSlot,
          total as u32,
          AGG_ALIGN_SHIFT,
        ));

        // Length prefix at offset 0.
        let len_v = builder.ins().iconst(tctx.ptr_ty, n as i64);

        builder.ins().stack_store(len_v, slot, 0);

        // Elements at `8 + i * 8`.
        for (i, eid) in elements.iter().enumerate() {
          let Some(v) = ctx.values.get(eid).copied() else {
            emit_exit_1(tctx, builder);

            ctx.terminated = true;

            return;
          };

          let offset =
            (AGG_SLOT_SIZE as i32) + ((i as u32) * AGG_SLOT_SIZE) as i32;

          builder.ins().stack_store(v, slot, offset);
        }

        let addr = builder.ins().stack_addr(tctx.ptr_ty, slot, 0);

        ctx.values.insert(*dst, addr);
      }
      Insn::TupleIndex {
        dst,
        tuple,
        index,
        ty_id,
      } => {
        // Compile-time indexed read: `tup.N`. Offset is
        // `index * 8` with the uniform slot layout.
        let Some(base) = ctx.values.get(tuple).copied() else {
          emit_exit_1(tctx, builder);

          ctx.terminated = true;

          return;
        };

        let field_ty = ty_id_to_clif(*ty_id, tctx.ptr_ty);
        let offset = (*index * AGG_SLOT_SIZE) as i32;
        let v = builder.ins().load(field_ty, MemFlags::new(), base, offset);

        ctx.values.insert(*dst, v);
      }
      Insn::FieldStore {
        base, index, value, ..
      } => {
        // Compile-time indexed write: `struct.N = value`. No
        // `dst` — purely side-effecting.
        let Some(base_addr) = ctx.values.get(base).copied() else {
          emit_exit_1(tctx, builder);

          ctx.terminated = true;

          return;
        };

        let Some(v) = ctx.values.get(value).copied() else {
          emit_exit_1(tctx, builder);

          ctx.terminated = true;

          return;
        };

        let offset = (*index * AGG_SLOT_SIZE) as i32;

        builder.ins().store(MemFlags::new(), v, base_addr, offset);
      }
      Insn::ArrayIndex {
        dst,
        array,
        index,
        ty_id,
      } => {
        // Runtime-indexed read: `arr[i]`. Data starts at
        // offset 8 because of the length prefix; compute the
        // element address as `base + 8 + (index << 3)`. Index
        // is widened to pointer width if narrower.
        let Some(base) = ctx.values.get(array).copied() else {
          emit_exit_1(tctx, builder);

          ctx.terminated = true;

          return;
        };

        let Some(idx_v) = ctx.values.get(index).copied() else {
          emit_exit_1(tctx, builder);

          ctx.terminated = true;

          return;
        };

        let idx_ext = widen_to_ptr(builder, idx_v, tctx.ptr_ty);
        let byte_off = builder.ins().ishl_imm(idx_ext, 3);
        let addr = builder.ins().iadd(base, byte_off);
        let elem_ty = ty_id_to_clif(*ty_id, tctx.ptr_ty);
        let v = builder.ins().load(
          elem_ty,
          MemFlags::new(),
          addr,
          AGG_SLOT_SIZE as i32,
        );

        ctx.values.insert(*dst, v);
      }
      Insn::ArrayStore {
        array,
        index,
        value,
        ..
      } => {
        // Runtime-indexed write: `arr[i] = value`. Same
        // length-prefix-aware addr calc as `ArrayIndex`.
        let Some(base) = ctx.values.get(array).copied() else {
          emit_exit_1(tctx, builder);

          ctx.terminated = true;

          return;
        };

        let Some(idx_v) = ctx.values.get(index).copied() else {
          emit_exit_1(tctx, builder);

          ctx.terminated = true;

          return;
        };

        let Some(v) = ctx.values.get(value).copied() else {
          emit_exit_1(tctx, builder);

          ctx.terminated = true;

          return;
        };

        let idx_ext = widen_to_ptr(builder, idx_v, tctx.ptr_ty);
        let byte_off = builder.ins().ishl_imm(idx_ext, 3);
        let addr = builder.ins().iadd(base, byte_off);

        builder
          .ins()
          .store(MemFlags::new(), v, addr, AGG_SLOT_SIZE as i32);
      }
      Insn::ArrayLen { dst, array, ty_id } => {
        // Length is the u64 LE prefix at offset 0. Load at the
        // SIR-declared return width (often `s32` / `int`) so
        // downstream ops that compare `i < len` see matched
        // operand types. LE storage means a narrower load
        // correctly takes the low bytes for lengths ≤ 2^width.
        let Some(base) = ctx.values.get(array).copied() else {
          emit_exit_1(tctx, builder);

          ctx.terminated = true;

          return;
        };

        let len_ty = ty_id_to_clif(*ty_id, tctx.ptr_ty);
        let v = builder.ins().load(len_ty, MemFlags::new(), base, 0);

        ctx.values.insert(*dst, v);
      }
      Insn::BinOp {
        dst,
        op,
        lhs,
        rhs,
        ty_id,
      } => {
        let Some(l) = ctx.values.get(lhs).copied() else {
          // Operand missing — the producing insn was in an
          // unimplemented arm that trapped. Emit trap + bail.
          emit_exit_1(tctx, builder);

          ctx.terminated = true;

          return;
        };

        let Some(r) = ctx.values.get(rhs).copied() else {
          emit_exit_1(tctx, builder);

          ctx.terminated = true;

          return;
        };

        // SIR's expected-type propagation + post-emit
        // resolve walker guarantee that `BinOp` operands
        // share a `ty_id` by codegen time. Cranelift's
        // type-homogeneous ops accept them directly.
        let v = translate_binop(tctx, builder, *op, l, r, *ty_id);

        ctx.values.insert(*dst, v);
      }
      Insn::UnOp {
        dst,
        op,
        rhs,
        ty_id,
      } => {
        let Some(r) = ctx.values.get(rhs).copied() else {
          emit_exit_1(tctx, builder);

          ctx.terminated = true;

          return;
        };

        let v = translate_unop(tctx, builder, *op, r, *ty_id);

        ctx.values.insert(*dst, v);
      }
      Insn::Call {
        dst, name, args, ..
      } => {
        // Intercept zo's I/O / assertion intrinsics before
        // the normal `declare_func_in_func` path — they have
        // no symbol in libc, so we inline them (through libc
        // `write` for show, `exit` for check) matching the
        // ARM backend's open-coded behavior.
        let name_str = tctx.interner.get(*name);

        if matches!(name_str, "show" | "showln" | "eshow" | "eshowln") {
          emit_io_intrinsic(tctx, builder, ctx, *dst, name_str, args);

          continue;
        }

        if name_str == "check" {
          emit_check_intrinsic(tctx, builder, ctx, *dst, args);

          continue;
        }

        let Some(func_id) = tctx.func_ids.get(name).copied() else {
          // Callee not in the first-pass declaration table —
          // semantic analyzer shouldn't let this through, but
          // trap rather than panic.
          emit_exit_1(tctx, builder);

          ctx.terminated = true;

          return;
        };

        // Gather CLIF args from the SSA map; bail to trap on
        // any missing producer (an upstream arm must have
        // trapped, leaving an operand undefined).
        let mut arg_vals: Vec<ir::Value> = Vec::with_capacity(args.len());

        for arg in args {
          let Some(v) = ctx.values.get(arg).copied() else {
            emit_exit_1(tctx, builder);

            ctx.terminated = true;

            return;
          };

          arg_vals.push(v);
        }

        // Import the callee's `FuncId` into the current
        // function (cranelift dedupes internally across repeat
        // imports). Works for both `Linkage::Export` (user-
        // defined) and `Linkage::Import` (FFI intrinsics).
        let fref = tctx.module.declare_func_in_func(func_id, builder.func);

        // Every arg's `ty_id` already matches the callee's
        // corresponding param: `begin_call_ctx` covers
        // non-generic calls, and the post-emit resolve
        // walker in the executor covers generic
        // monomorphizations.
        let call = builder.ins().call(fref, &arg_vals);

        // Unit-return callees have an empty results vec. SIR
        // still carries a `dst: ValueId`, so materialize an
        // I8 0 sentinel — matches Appendix B row 1 of the
        // plan. Downstream refs for unit values are a
        // semantic bug and will never be read.
        let results = builder.inst_results(call);
        let v = if results.is_empty() {
          builder.ins().iconst(ir::types::I8, 0)
        } else {
          results[0]
        };

        ctx.values.insert(*dst, v);
      }
      Insn::Return { value, .. } => {
        // Resolve the explicit return value (if any) from the
        // SSA map; fall back to `vec![]`.
        let explicit: Vec<ir::Value> = value
          .and_then(|v| ctx.values.get(&v).copied())
          .map_or_else(Vec::new, |v| vec![v]);

        // `main` must return an `i32` to match the CLIF
        // signature we built (`() -> i32`). If the zo source
        // didn't provide a return value (`fun main()` with
        // no explicit `return`), inject a 0 sentinel so the
        // process exits cleanly instead of reading junk from
        // the return register.
        let rets: Vec<ir::Value> = if ctx.is_main && explicit.is_empty() {
          vec![builder.ins().iconst(ir::types::I32, 0)]
        } else {
          explicit
        };

        builder.ins().return_(&rets);

        ctx.terminated = true;

        // Any insn after a Return is dead. Create + switch to
        // a stray block so following emissions don't panic
        // the builder — Cranelift DCEs unreachable blocks.
        let dead = builder.create_block();

        builder.switch_to_block(dead);

        ctx.terminated = false;
      }
      Insn::Label { id } => {
        let block = *ctx
          .blocks
          .entry(*id)
          .or_insert_with(|| builder.create_block());

        // Fall-through from the previous block: if it has no
        // terminator yet, synthesize a jump into this label's
        // block. Matches the ARM path's implicit fall-through
        // but keeps CLIF's "every block ends with a terminator"
        // invariant.
        seal_current_with_jump(builder, ctx, block);
        builder.switch_to_block(block);
      }
      Insn::Jump { target } => {
        // Lazy-create the target block if the executor never
        // emitted a matching `Label` for it. Happens in
        // optimized else-chains where a merge-point id is
        // reserved but the label gets dropped.
        let block = *ctx
          .blocks
          .entry(*target)
          .or_insert_with(|| builder.create_block());

        builder.ins().jump(block, &[]);

        ctx.terminated = true;

        // Fresh block for any stray insns before the next
        // `Label`. CLIF requires a valid current block after
        // a terminator.
        let dead = builder.create_block();

        builder.switch_to_block(dead);

        ctx.terminated = false;
      }
      Insn::BranchIfNot { cond, target } => {
        let Some(cond_v) = ctx.values.get(cond).copied() else {
          emit_exit_1(tctx, builder);

          ctx.terminated = true;

          return;
        };

        let target_block = *ctx
          .blocks
          .entry(*target)
          .or_insert_with(|| builder.create_block());
        let fallthrough = builder.create_block();

        // SIR semantics: branch to `target` if cond == 0.
        // Cranelift's `brif(cond, then, then_args, else, else_args)`
        // goes to `then` when cond != 0 — so `then` is the
        // fallthrough and `else` is the SIR target.
        builder
          .ins()
          .brif(cond_v, fallthrough, &[], target_block, &[]);

        builder.switch_to_block(fallthrough);
      }
      Insn::VarDef {
        name, ty_id, init, ..
      } => {
        // `init: Some(vid)` maps to an immediate `def_var` so
        // the following `Load`s see the initializer. `None`
        // leaves the Variable declared-but-undefined — a later
        // `Store` must hit it before any `Load`, which the
        // semantic analyzer already enforces.
        //
        // The declared CLIF type follows the init value's
        // type when present — SIR's `ty_id` can be an alias
        // (e.g. a generic param bound, a type-alias) whose
        // `ty_id_to_clif` fallback width would mismatch the
        // init value's actual width.
        let declared_ty = ty_id_to_clif(*ty_id, ir::types::I64);
        let init_value = init.and_then(|v| ctx.values.get(&v).copied());
        let ty = init_value
          .map(|v| builder.func.dfg.value_type(v))
          .unwrap_or(declared_ty);
        let var = ctx.declare_local(builder, *name, ty);

        if init.is_some() && init_value.is_none() {
          emit_exit_1(tctx, builder);

          ctx.terminated = true;

          return;
        }

        if let Some(v) = init_value {
          builder.def_var(var, v);
        }
      }
      Insn::Store { name, value, .. } => {
        let Some(v) = ctx.values.get(value).copied() else {
          emit_exit_1(tctx, builder);

          ctx.terminated = true;

          return;
        };

        // Auto-declare on first sight. The executor emits
        // synthetic locals like `__branch_result_0__` for
        // branch / ternary result slots without a preceding
        // `VarDef` — they're Stored to and Loaded from
        // directly. Fall back to declaring the Variable the
        // first time we see one, sized to the actual value
        // being stored (the Insn's `ty_id` can be an alias
        // whose width doesn't match the stored value).
        let var = match ctx.vars.get(name).copied() {
          Some(var) => var,
          None => {
            let ty = builder.func.dfg.value_type(v);

            ctx.declare_local(builder, *name, ty)
          }
        };

        builder.def_var(var, v);
      }
      Insn::Load { dst, src, .. } => {
        // Resolution order for `Local(sym)`:
        //   1. `const_defs` — module-scope `val NAME = lit;`.
        //      Inline the raw literal so every use site emits
        //      a fresh iconst / data-section reference. Plan
        //      row 6.
        //   2. `vars[sym]` — locals declared by `VarDef` and
        //      parameters mirrored under their name.
        // `Param(idx)` skips the const-def check (params are
        // never module-scope constants) and goes straight to
        // the index-keyed `params` vec.
        if let LoadSource::Local(sym) = src
          && let Some(lit) = tctx.const_defs.get(sym).copied()
        {
          let v = materialize_const_literal(tctx, builder, lit);

          ctx.values.insert(*dst, v);

          continue;
        }

        let var = match src {
          LoadSource::Param(idx) => ctx.params.get(*idx as usize).copied(),
          LoadSource::Local(sym) => ctx.vars.get(sym).copied(),
        };
        let Some(var) = var else {
          emit_exit_1(tctx, builder);

          ctx.terminated = true;

          return;
        };

        let v = builder.use_var(var);

        ctx.values.insert(*dst, v);
      }
      Insn::Cast {
        dst,
        src,
        from_ty,
        to_ty,
      } => {
        let Some(src_v) = ctx.values.get(src).copied() else {
          emit_exit_1(tctx, builder);

          ctx.terminated = true;

          return;
        };

        let v = translate_cast(builder, src_v, *from_ty, *to_ty, tctx.ptr_ty);

        ctx.values.insert(*dst, v);
      }
      Insn::Nop => {}
      // Module-level markers that the executor interleaves
      // with real work. No-ops in this backend: registration
      // already happened in the top-level first pass.
      // `Directive` (`#run`, `#dom`) is a semantic marker —
      // no CLIF to emit.
      Insn::PackDecl { .. }
      | Insn::ModuleLoad { .. }
      | Insn::EnumDef { .. }
      | Insn::StructDef { .. }
      | Insn::ArrayTyDef { .. }
      | Insn::MapTyDef { .. }
      | Insn::ConstDef { .. }
      | Insn::Directive { .. } => {}
      // Catch-all for insns not yet implemented (`Template`,
      // `ArrayPush`, `ArrayPop`, etc.): exit the process with
      // code 1 and bail out of this body. The module still
      // builds; calling the stubbed fn at runtime terminates
      // cleanly instead of hanging on a raw `ud2` under
      // Rosetta.
      _ => {
        emit_exit_1(tctx, builder);

        ctx.terminated = true;

        return;
      }
    }
  }
}

/// Translates a SIR [`BinOp`] — dispatches on the operator
/// and (via `ty_id`) on integer-vs-float and signed-vs-
/// unsigned. Signed int is the default; unsigned is detected
/// through `is_unsigned_int(ty_id)` (TyIds 11..=14), float
/// through `is_float(ty_id)` (15..=17).
fn translate_binop(
  tctx: &mut TCtx<'_>,
  builder: &mut FunctionBuilder,
  op: BinOp,
  l: ir::Value,
  r: ir::Value,
  ty_id: TyId,
) -> ir::Value {
  let unsigned = is_unsigned_int(ty_id);
  let fp = is_float(ty_id);

  match op {
    BinOp::Add => {
      if fp {
        builder.ins().fadd(l, r)
      } else {
        builder.ins().iadd(l, r)
      }
    }
    BinOp::Sub => {
      if fp {
        builder.ins().fsub(l, r)
      } else {
        builder.ins().isub(l, r)
      }
    }
    BinOp::Mul => {
      if fp {
        builder.ins().fmul(l, r)
      } else {
        builder.ins().imul(l, r)
      }
    }
    BinOp::Div => {
      if fp {
        builder.ins().fdiv(l, r)
      } else if unsigned {
        builder.ins().udiv(l, r)
      } else {
        builder.ins().sdiv(l, r)
      }
    }
    BinOp::Rem => {
      if unsigned {
        builder.ins().urem(l, r)
      } else {
        builder.ins().srem(l, r)
      }
      // Float rem falls through to int path today — zo
      // doesn't generate `%` on floats in current programs.
      // If that changes, route through a libm `fmod` FFI.
    }
    BinOp::And | BinOp::BitAnd => builder.ins().band(l, r),
    BinOp::Or | BinOp::BitOr => builder.ins().bor(l, r),
    BinOp::BitXor => builder.ins().bxor(l, r),
    BinOp::Shl => builder.ins().ishl(l, r),
    BinOp::Shr => {
      if unsigned {
        builder.ins().ushr(l, r)
      } else {
        builder.ins().sshr(l, r)
      }
    }
    BinOp::Eq => {
      if fp {
        builder.ins().fcmp(FloatCC::Equal, l, r)
      } else {
        builder.ins().icmp(IntCC::Equal, l, r)
      }
    }
    BinOp::Neq => {
      if fp {
        builder.ins().fcmp(FloatCC::NotEqual, l, r)
      } else {
        builder.ins().icmp(IntCC::NotEqual, l, r)
      }
    }
    BinOp::Lt => {
      if fp {
        builder.ins().fcmp(FloatCC::LessThan, l, r)
      } else if unsigned {
        builder.ins().icmp(IntCC::UnsignedLessThan, l, r)
      } else {
        builder.ins().icmp(IntCC::SignedLessThan, l, r)
      }
    }
    BinOp::Lte => {
      if fp {
        builder.ins().fcmp(FloatCC::LessThanOrEqual, l, r)
      } else if unsigned {
        builder.ins().icmp(IntCC::UnsignedLessThanOrEqual, l, r)
      } else {
        builder.ins().icmp(IntCC::SignedLessThanOrEqual, l, r)
      }
    }
    BinOp::Gt => {
      if fp {
        builder.ins().fcmp(FloatCC::GreaterThan, l, r)
      } else if unsigned {
        builder.ins().icmp(IntCC::UnsignedGreaterThan, l, r)
      } else {
        builder.ins().icmp(IntCC::SignedGreaterThan, l, r)
      }
    }
    BinOp::Gte => {
      if fp {
        builder.ins().fcmp(FloatCC::GreaterThanOrEqual, l, r)
      } else if unsigned {
        builder.ins().icmp(IntCC::UnsignedGreaterThanOrEqual, l, r)
      } else {
        builder.ins().icmp(IntCC::SignedGreaterThanOrEqual, l, r)
      }
    }
    BinOp::Concat => {
      // Route through the `zo_str_concat` runtime helper in
      // `zo-linker/src/runtime.c`. The result is a fresh
      // `[u64 LE len, bytes]` buffer that composes with
      // every other zo string path.
      let fid = ensure_libc_func(tctx, "zo_str_concat", |ptr_ty, cc| {
        let mut sig = ir::Signature::new(cc);

        sig.params.push(AbiParam::new(ptr_ty));
        sig.params.push(AbiParam::new(ptr_ty));
        sig.returns.push(AbiParam::new(ptr_ty));

        sig
      });
      let fref = tctx.module.declare_func_in_func(fid, builder.func);
      let call = builder.ins().call(fref, &[l, r]);

      builder.inst_results(call)[0]
    }
  }
}

/// Translates a SIR [`UnOp`].
fn translate_unop(
  _tctx: &mut TCtx<'_>,
  builder: &mut FunctionBuilder,
  op: UnOp,
  r: ir::Value,
  ty_id: TyId,
) -> ir::Value {
  match op {
    UnOp::Neg => {
      if is_float(ty_id) {
        builder.ins().fneg(r)
      } else {
        builder.ins().ineg(r)
      }
    }
    UnOp::Not => {
      // Boolean not: r xor 1. Assumes bool is canonical 0/1
      // in I8 (enforced everywhere CLIF produces bools).
      builder.ins().bxor_imm(r, 1)
    }
    UnOp::BitNot => builder.ins().bnot(r),
  }
}
