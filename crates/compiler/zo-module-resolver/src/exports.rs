use crate::resolver::translate_symbol;

use zo_interner::{Interner, Symbol};
use zo_sir::{Insn, Sir};
use zo_ty::TyId;
use zo_ty_checker::TyChecker;
use zo_value::{FunDef, Pubness, ValueId};

/// An exported compile-time constant from a module.
#[derive(Clone, Debug)]
pub struct ExportedVar {
  /// The name of the constant (re-interned).
  pub name: Symbol,
  /// The type of the constant.
  pub ty_id: TyId,
  /// The initializer value (if compile-time known).
  pub init: Option<ValueId>,
}

/// Exported symbols from a compiled module.
pub struct ModuleExports {
  /// The function definitions (re-interned symbols).
  pub funs: Vec<FunDef>,
  /// The constant definitions (re-interned symbols).
  pub vars: Vec<ExportedVar>,
  /// The SIR instruction stream for codegen merging.
  pub sir_instructions: Vec<Insn>,
  /// The next value id (for ValueId offset).
  pub next_value_id: u32,
}

/// Translates a TyId from one TyChecker to another.
///
/// Resolves the `Ty` value in the source checker, then interns
/// it in the destination checker. For pre-registered primitives
/// this is a no-op (same ID). For complex types this remaps.
pub fn translate_ty_id(
  src_id: TyId,
  src_checker: &TyChecker,
  dst_checker: &mut TyChecker,
) -> TyId {
  let ty = src_checker.resolve_ty(src_id);
  dst_checker.intern_ty(ty)
}

/// Extracts pub exports from a compiled module's SIR.
///
/// Translates symbol names and TyIds from the module's
/// interner/type checker into the caller's.
///
/// If `selective` is `Some(name)`, only the matching export
/// is included.
pub fn extract_exports(
  sir: Sir,
  selective: Option<&str>,
  src_interner: &Interner,
  dst_interner: &mut Interner,
  src_ty_checker: &TyChecker,
  dst_ty_checker: &mut TyChecker,
) -> ModuleExports {
  let mut funs = Vec::new();
  let mut vars = Vec::new();

  for insn in &sir.instructions {
    match insn {
      Insn::FunDef {
        name,
        params,
        return_ty,
        body_start,
        kind,
        pubness,
      } => {
        if *pubness != Pubness::Yes {
          continue;
        }

        let src_name = src_interner.get(*name);

        if let Some(filter) = selective
          && src_name != filter
        {
          continue;
        }

        let dst_name = dst_interner.intern(src_name);
        let dst_params = params
          .iter()
          .map(|(p, ty)| {
            (
              translate_symbol(*p, src_interner, dst_interner),
              translate_ty_id(*ty, src_ty_checker, dst_ty_checker),
            )
          })
          .collect::<Vec<_>>();

        let dst_return_ty =
          translate_ty_id(*return_ty, src_ty_checker, dst_ty_checker);

        funs.push(FunDef {
          name: dst_name,
          params: dst_params,
          return_ty: dst_return_ty,
          body_start: *body_start,
          kind: *kind,
          pubness: *pubness,
          type_params: Vec::new(),
        });
      }

      Insn::VarDef {
        name,
        ty_id,
        init,
        pubness,
        ..
      } => {
        if *pubness != Pubness::Yes {
          continue;
        }

        let src_name = src_interner.get(*name);

        if let Some(filter) = selective
          && src_name != filter
        {
          continue;
        }

        let dst_name = dst_interner.intern(src_name);
        let dst_ty_id = translate_ty_id(*ty_id, src_ty_checker, dst_ty_checker);

        vars.push(ExportedVar {
          name: dst_name,
          ty_id: dst_ty_id,
          init: *init,
        });
      }

      _ => {}
    }
  }

  // Filter out PackDecl — only code-generating
  // instructions should be merged into the user's SIR.
  let sir_instructions = sir
    .instructions
    .into_iter()
    .filter(|i| !matches!(i, Insn::PackDecl { .. }))
    .collect();

  ModuleExports {
    funs,
    vars,
    sir_instructions,
    next_value_id: sir.next_value_id,
  }
}
