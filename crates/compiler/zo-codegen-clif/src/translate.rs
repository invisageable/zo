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
use zo_sir::{BinOp, Insn, UnOp};
use zo_ty::TyId;
use zo_value::{FunctionKind, ValueId};

use cranelift::codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift::codegen::ir::{AbiParam, Function, InstBuilder, UserFuncName};
use cranelift::codegen::isa::CallConv;
use cranelift::codegen::{Context, ir};
use cranelift::frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_module::{Linkage, Module};
use cranelift_object::ObjectModule;
use rustc_hash::FxHashMap as HashMap;

/// Per-function translation state. A fresh [`FunCtx`] is built
/// for every SIR `FunDef`.
pub(crate) struct FunCtx {
  /// SIR `ValueId` → CLIF `Value`.
  pub(crate) values: HashMap<ValueId, ir::Value>,
  /// SIR label id → CLIF `Block` (populated by phase 2c).
  #[allow(dead_code)]
  pub(crate) blocks: HashMap<u32, ir::Block>,
}

impl FunCtx {
  fn new() -> Self {
    Self {
      values: HashMap::default(),
      blocks: HashMap::default(),
    }
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

    translate_body(&mut builder, &mut fun_ctx, &insns[body_start_u..end]);

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
/// [`FunctionBuilder`].
fn translate_body(
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

          return;
        };
        let Some(r) = ctx.values.get(rhs).copied() else {
          builder.ins().trap(ir::TrapCode::user(1).unwrap());

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

          return;
        };

        let v = translate_unop(builder, *op, r, *ty_id);

        ctx.values.insert(*dst, v);
      }
      Insn::Return { value, .. } => {
        let rets: Vec<ir::Value> = value
          .and_then(|v| ctx.values.get(&v).copied())
          .map_or_else(Vec::new, |v| vec![v]);

        builder.ins().return_(&rets);
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
      | Insn::ConstDef { .. }
      | Insn::VarDef { .. } => {}
      // Phase 2a stub: any insn not yet supported emits a
      // single `trap` terminator and bails out of this body.
      // The module still builds; calling the stubbed fn at
      // runtime will abort. Filled in progressively by 2b/c/d.
      _ => {
        builder
          .ins()
          .trap(cranelift::codegen::ir::TrapCode::user(1).unwrap());

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
