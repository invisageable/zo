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

use crate::types::{pointer_ty, ty_id_to_clif};

use zo_interner::{Interner, Symbol};
use zo_sir::Insn;
use zo_ty::TyId;
use zo_value::{FunctionKind, ValueId};

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
