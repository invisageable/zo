use crate::resolver::translate_symbol;

use zo_interner::{Interner, Symbol};
use zo_sir::{Insn, Sir};
use zo_ty::TyId;
use zo_value::{FunDef, ValueId};

/// An exported compile-time constant from a module.
#[derive(Clone, Debug)]
pub struct ExportedVar {
  pub name: Symbol,
  pub ty_id: TyId,
  pub init: Option<ValueId>,
}

/// Exported symbols from a compiled module.
pub struct ModuleExports {
  /// Public function definitions (symbols re-interned).
  pub funs: Vec<FunDef>,
  /// Public constant definitions (symbols re-interned).
  pub vars: Vec<ExportedVar>,
  /// Full SIR instruction stream for codegen merging.
  pub sir_instructions: Vec<Insn>,
  /// Module's next_value_id (for ValueId offset).
  pub next_value_id: u32,
}

/// Extracts pub exports from a compiled module's SIR.
///
/// Scans for `Insn::FunDef` and `Insn::VarDef` with
/// `is_pub: true` and translates symbol names from the
/// module's interner into the caller's. If `selective` is
/// `Some(name)`, only the matching export is included.
pub fn extract_exports(
  sir: &Sir,
  selective: Option<&str>,
  src_interner: &Interner,
  dst_interner: &mut Interner,
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
        is_intrinsic,
        is_pub,
      } => {
        if !is_pub {
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
            (translate_symbol(*p, src_interner, dst_interner), *ty)
          })
          .collect::<Vec<_>>();

        funs.push(FunDef {
          name: dst_name,
          params: dst_params,
          return_ty: *return_ty,
          body_start: *body_start,
          is_intrinsic: *is_intrinsic,
          is_pub: *is_pub,
        });
      }

      Insn::VarDef {
        name,
        ty_id,
        init,
        is_pub,
        ..
      } => {
        if !is_pub {
          continue;
        }

        let src_name = src_interner.get(*name);

        if let Some(filter) = selective
          && src_name != filter
        {
          continue;
        }

        let dst_name = dst_interner.intern(src_name);

        vars.push(ExportedVar {
          name: dst_name,
          ty_id: *ty_id,
          init: *init,
        });
      }

      _ => {}
    }
  }

  ModuleExports {
    funs,
    vars,
    sir_instructions: sir.instructions.clone(),
    next_value_id: sir.next_value_id,
  }
}
