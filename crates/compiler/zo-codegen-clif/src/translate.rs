//! SIR → CLIF instruction translator.
//!
//! Per-function pipeline:
//!   1. Determine body range `[body_start .. next_fundef_or_end]`.
//!   2. Label pre-pass — allocate CLIF blocks per `Label { id }`
//!      (phase 2c).
//!   3. Main pass — walk instructions, emit CLIF.
//!
//! Phase 2a coverage: `FunDef`, `ConstInt`, `Return`. Every
//! other value-producing variant is a `todo!()` marker so
//! missing coverage surfaces at test time instead of silently
//! emitting wrong code. Module-level markers (`PackDecl`,
//! `ModuleLoad`, type definitions) are explicit no-ops.

use crate::types::{is_float, is_unsigned_int, pointer_ty, ty_id_to_clif};

use zo_interner::{Interner, Symbol};
use zo_sir::{BinOp, Insn, LoadSource, UnOp};
use zo_ty::TyId;
use zo_value::{FunctionKind, ValueId};

use cranelift::codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift::codegen::ir::{AbiParam, Function, InstBuilder, UserFuncName};
use cranelift::codegen::isa::CallConv;
use cranelift::codegen::{Context, ir};
use cranelift::frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
use cranelift_module::{Linkage, Module};
use cranelift_object::ObjectModule;
use rustc_hash::FxHashMap as HashMap;

/// Per-function translation state. A fresh [`FunCtx`] is built
/// for every SIR `FunDef`.
pub(crate) struct FunCtx {
  /// SIR `ValueId` → CLIF `Value`.
  pub(crate) values: HashMap<ValueId, ir::Value>,
  /// SIR label id → CLIF `Block` (populated by the label
  /// pre-pass so forward jumps can resolve).
  pub(crate) blocks: HashMap<u32, ir::Block>,
  /// Symbol-keyed slots — locals declared by `Insn::VarDef`
  /// plus parameters mirrored under their declaration name so
  /// `Store { name }` resolves regardless of whether it hits
  /// a param or a local. Cranelift's `Variable` API bridges
  /// SIR's mutable named slots and CLIF's SSA: `def_var` for
  /// writes, `use_var` for reads, phi nodes auto-inserted.
  pub(crate) vars: HashMap<Symbol, Variable>,
  /// Parameters in declaration order. `Load { Param(idx) }`
  /// indexes this vec directly (SIR's `Param` carries a u32
  /// slot index, not a symbol, so name-keyed lookup can't
  /// serve it).
  pub(crate) params: Vec<Variable>,
  /// True iff the current CLIF block has a terminator already
  /// emitted (`return_`, `jump`, `brif`, `trap`). CLIF's
  /// `FunctionBuilder::is_filled` is private, so we mirror the
  /// state here — set by every terminator emission, reset on
  /// `switch_to_block`. Used at each `Label` to decide whether
  /// a fall-through jump needs synthesizing.
  pub(crate) terminated: bool,
}

impl FunCtx {
  fn new() -> Self {
    Self {
      values: HashMap::default(),
      blocks: HashMap::default(),
      vars: HashMap::default(),
      params: Vec::new(),
      terminated: false,
    }
  }

  /// Declares a fresh [`Variable`] for `name` with CLIF type
  /// `ty` on the builder and records the mapping. Cranelift
  /// mints the `Variable` internally (`declare_var` returns
  /// it); we just thread it into `vars`. Called for every
  /// parameter at entry and every `VarDef`.
  fn declare_local(
    &mut self,
    builder: &mut FunctionBuilder,
    name: Symbol,
    ty: ir::Type,
  ) -> Variable {
    let var = builder.declare_var(ty);

    self.vars.insert(name, var);

    var
  }
}

/// Translates the whole SIR instruction stream into the given
/// [`ObjectModule`]. One CLIF function per SIR `FunDef`.
pub(crate) fn translate_module(
  module: &mut ObjectModule,
  interner: &Interner,
  insns: &[Insn],
) {
  let call_conv = module.target_config().default_call_conv;
  let ptr_ty = pointer_ty(module);

  // First pass: declare every function so forward calls
  // inside bodies can resolve their `FuncId`.
  let mut func_ids: HashMap<Symbol, cranelift_module::FuncId> =
    HashMap::default();

  for insn in insns {
    if let Insn::FunDef {
      name,
      params,
      return_ty,
      kind,
      ..
    } = insn
    {
      let linkage = match kind {
        FunctionKind::Intrinsic => Linkage::Import,
        _ => Linkage::Export,
      };

      let sig = build_signature(params, *return_ty, call_conv, ptr_ty);
      let fname = interner.get(*name);

      let func_id = module
        .declare_function(fname, linkage, &sig)
        .expect("declare_function failed");

      func_ids.insert(*name, func_id);
    }
  }

  // Second pass: define each non-intrinsic function's body.
  let mut i = 0;

  while i < insns.len() {
    let Insn::FunDef {
      name,
      params,
      return_ty,
      body_start,
      kind,
      ..
    } = &insns[i]
    else {
      i += 1;
      continue;
    };

    // Body range for this function. Intrinsic FFI functions
    // have `body_start = 0` (sentinel, no body), so start the
    // scan at the insn RIGHT AFTER the current FunDef to
    // avoid re-finding itself.
    let body_start_u = (*body_start as usize).max(i + 1);
    let end = next_fundef_after(insns, i + 1).unwrap_or(insns.len());

    // Intrinsic functions have no body — declared above as
    // imports; no CLIF function to define.
    if matches!(kind, FunctionKind::Intrinsic) {
      i = end;
      continue;
    }

    let func_id = func_ids[name];
    let sig = build_signature(params, *return_ty, call_conv, ptr_ty);

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

    let mut fun_ctx = FunCtx::new();

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

    // Phase 2c label pre-pass: allocate a CLIF block per SIR
    // `Label { id }` in the body so forward jumps / brifs can
    // reference them before we walk past. Without this, a
    // `Jump { target: 0 }` emitted at SIR index 5 can't
    // reference the block for `Label { id: 0 }` at index 12.
    preallocate_label_blocks(
      &mut builder,
      &mut fun_ctx,
      &insns[body_start_u..end],
    );

    translate_body(
      module,
      &func_ids,
      &mut builder,
      &mut fun_ctx,
      &insns[body_start_u..end],
    );

    // Seal every block — tells cranelift all predecessors are
    // now known, finalizes any pending SSA phi nodes.
    builder.seal_all_blocks();
    builder.finalize();

    module
      .define_function(func_id, &mut ctx)
      .expect("define_function failed");

    i = end;
  }
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
) -> ir::Signature {
  let mut sig = ir::Signature::new(call_conv);

  for (_, pty) in params {
    sig.params.push(AbiParam::new(ty_id_to_clif(*pty, ptr_ty)));
  }

  if return_ty != TyId(1) {
    sig
      .returns
      .push(AbiParam::new(ty_id_to_clif(return_ty, ptr_ty)));
  }

  sig
}

/// Walks the body instructions and emits CLIF via the given
/// [`FunctionBuilder`]. `module` + `func_ids` are threaded in
/// to let `Insn::Call` resolve its callee's `FuncId` and
/// import it into the current function via
/// `declare_func_in_func`.
fn translate_body(
  module: &mut ObjectModule,
  func_ids: &HashMap<Symbol, cranelift_module::FuncId>,
  builder: &mut FunctionBuilder,
  ctx: &mut FunCtx,
  body: &[Insn],
) {
  for insn in body {
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
          builder.ins().trap(ir::TrapCode::user(1).unwrap());

          ctx.terminated = true;

          return;
        };
        let Some(r) = ctx.values.get(rhs).copied() else {
          builder.ins().trap(ir::TrapCode::user(1).unwrap());

          ctx.terminated = true;

          return;
        };

        let v = translate_binop(builder, *op, l, r, *ty_id);

        ctx.values.insert(*dst, v);
      }
      Insn::UnOp {
        dst,
        op,
        rhs,
        ty_id,
      } => {
        let Some(r) = ctx.values.get(rhs).copied() else {
          builder.ins().trap(ir::TrapCode::user(1).unwrap());

          ctx.terminated = true;

          return;
        };

        let v = translate_unop(builder, *op, r, *ty_id);

        ctx.values.insert(*dst, v);
      }
      Insn::Call {
        dst, name, args, ..
      } => {
        let Some(func_id) = func_ids.get(name).copied() else {
          // Callee not in the first-pass declaration table —
          // semantic analyzer shouldn't let this through, but
          // trap rather than panic.
          builder.ins().trap(ir::TrapCode::user(1).unwrap());

          ctx.terminated = true;

          return;
        };

        // Gather CLIF args from the SSA map; bail to trap on
        // any missing producer (an upstream arm must have
        // trapped, leaving an operand undefined).
        let mut arg_vals: Vec<ir::Value> = Vec::with_capacity(args.len());

        for arg in args {
          let Some(v) = ctx.values.get(arg).copied() else {
            builder.ins().trap(ir::TrapCode::user(1).unwrap());

            ctx.terminated = true;

            return;
          };

          arg_vals.push(v);
        }

        // Import the callee's `FuncId` into the current
        // function (cranelift dedupes internally across repeat
        // imports). Works for both `Linkage::Export` (user-
        // defined) and `Linkage::Import` (FFI intrinsics).
        let fref = module.declare_func_in_func(func_id, builder.func);
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
        let rets: Vec<ir::Value> = value
          .and_then(|v| ctx.values.get(&v).copied())
          .map_or_else(Vec::new, |v| vec![v]);

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
        let block = ctx.blocks[id];

        // Fall-through from the previous block: if it has no
        // terminator yet, synthesize a jump into this label's
        // block. Matches the ARM path's implicit fall-through
        // but keeps CLIF's "every block ends with a terminator"
        // invariant.
        seal_current_with_jump(builder, ctx, block);
        builder.switch_to_block(block);
      }
      Insn::Jump { target } => {
        let block = ctx.blocks[target];

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
          builder.ins().trap(ir::TrapCode::user(1).unwrap());

          ctx.terminated = true;

          return;
        };

        let target_block = ctx.blocks[target];
        let fallthrough = builder.create_block();

        // SIR semantics: branch to `target` iff cond == 0.
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
        let ty = ty_id_to_clif(*ty_id, ir::types::I64);
        let var = ctx.declare_local(builder, *name, ty);

        // `init: Some(vid)` maps to an immediate `def_var` so
        // the following `Load`s see the initializer. `None`
        // leaves the Variable declared-but-undefined — a later
        // `Store` must hit it before any `Load`, which the
        // semantic analyzer already enforces.
        if let Some(init_v) = init {
          let Some(v) = ctx.values.get(init_v).copied() else {
            builder.ins().trap(ir::TrapCode::user(1).unwrap());

            ctx.terminated = true;

            return;
          };

          builder.def_var(var, v);
        }
      }
      Insn::Store { name, value, .. } => {
        let Some(var) = ctx.vars.get(name).copied() else {
          builder.ins().trap(ir::TrapCode::user(1).unwrap());

          ctx.terminated = true;

          return;
        };
        let Some(v) = ctx.values.get(value).copied() else {
          builder.ins().trap(ir::TrapCode::user(1).unwrap());

          ctx.terminated = true;

          return;
        };

        builder.def_var(var, v);
      }
      Insn::Load { dst, src, .. } => {
        // `Param(idx)` hits `params[idx]`; `Local(sym)` hits
        // `vars[sym]`. Both routes resolve to a `Variable`
        // which `use_var` materializes into an SSA value,
        // auto-inserting phis at merges.
        let var = match src {
          LoadSource::Param(idx) => ctx.params.get(*idx as usize).copied(),
          LoadSource::Local(sym) => ctx.vars.get(sym).copied(),
        };
        let Some(var) = var else {
          builder.ins().trap(ir::TrapCode::user(1).unwrap());

          ctx.terminated = true;

          return;
        };

        let v = builder.use_var(var);

        ctx.values.insert(*dst, v);
      }
      Insn::Nop => {}
      // Module-level markers that the executor interleaves
      // with real work. No-ops in this backend: registration
      // already happened in the top-level first pass.
      Insn::PackDecl { .. }
      | Insn::ModuleLoad { .. }
      | Insn::EnumDef { .. }
      | Insn::StructDef { .. }
      | Insn::ArrayTyDef { .. }
      | Insn::ConstDef { .. } => {}
      // Phase 2a stub: any insn not yet supported emits a
      // single `trap` terminator and bails out of this body.
      // The module still builds; calling the stubbed fn at
      // runtime will abort. Filled in progressively by 2b/c/d.
      _ => {
        builder
          .ins()
          .trap(cranelift::codegen::ir::TrapCode::user(1).unwrap());

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
    // Concat is a string op — lowered as an FFI helper call
    // in phase 2f. Trap until then.
    BinOp::Concat => {
      builder.ins().trap(ir::TrapCode::user(1).unwrap());

      // Unreachable: trap is a terminator. Return a dummy
      // value to satisfy the type checker — the caller will
      // discard it because the block is now dead.
      builder.ins().iconst(ir::types::I64, 0)
    }
  }
}

/// Translates a SIR [`UnOp`].
fn translate_unop(
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
    // Ref / Deref — phase 2d (locals / aggregates). Trap for now.
    UnOp::Ref | UnOp::Deref => {
      builder.ins().trap(ir::TrapCode::user(1).unwrap());

      builder.ins().iconst(ir::types::I64, 0)
    }
  }
}
