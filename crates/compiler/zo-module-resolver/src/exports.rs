use zo_interner::{Interner, Symbol};
use zo_sir::{Insn, Sir};
use zo_ty::TyId;
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

/// Exported enum definition for cross-module import. Carries
/// raw variant data instead of TyChecker-internal IDs so the
/// importing executor can re-intern into its own TyChecker.
pub struct ExportedEnum {
  pub name: Symbol,
  pub variants: Vec<(Symbol, u32, Vec<TyId>)>,
}

/// Exported struct definition for cross-module import.
pub struct ExportedStruct {
  pub name: Symbol,
  pub ty_id: TyId,
  pub fields: Vec<(Symbol, TyId, bool)>,
}

/// Exported compile-time constant (`val`).
pub struct ExportedConst {
  pub name: Symbol,
  pub ty_id: TyId,
  pub value: ValueId,
}

/// Exported symbols from a compiled module.
pub struct ModuleExports {
  /// The function definitions.
  pub funs: Vec<FunDef>,
  /// The variable definitions (imu/mut).
  pub vars: Vec<ExportedVar>,
  /// The enum definitions.
  pub enums: Vec<ExportedEnum>,
  /// The struct definitions.
  pub structs: Vec<ExportedStruct>,
  /// The compile-time constants (val).
  pub consts: Vec<ExportedConst>,
  /// The SIR instruction stream for codegen merging.
  pub sir_instructions: Vec<Insn>,
  /// The next value id (for ValueId offset).
  pub next_value_id: u32,
  /// The next label id (for Label / Jump / BranchIfNot
  /// offset when merging into the main SIR).
  pub next_label_id: u32,
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
  interner: &Interner,
  src_funs: &[zo_value::FunDef],
) -> ModuleExports {
  let mut funs = Vec::new();
  let mut vars = Vec::new();
  let mut enums = Vec::new();
  let mut structs = Vec::new();
  let mut consts = Vec::new();

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

        let fn_name = interner.get(*name);

        if let Some(filter) = selective
          && fn_name != filter
        {
          continue;
        }

        // Shared interner: symbols are already in the same
        // namespace — no translation needed. TyIds still need
        // translation until TyChecker is shared.
        let dst_params =
          params.iter().map(|(p, ty)| (*p, *ty)).collect::<Vec<_>>();

        let dst_return_ty = *return_ty;

        // Carry return_type_args as-is — they're Ty values
        // (not TyIds) so they don't need translation.
        let rta = src_funs
          .iter()
          .find(|f| f.name == *name)
          .map(|f| f.return_type_args.clone())
          .unwrap_or_default();

        funs.push(FunDef {
          name: *name,
          params: dst_params,
          return_ty: dst_return_ty,
          body_start: *body_start,
          kind: *kind,
          pubness: *pubness,
          type_params: Vec::new(),
          return_type_args: rta,
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

        let var_name = interner.get(*name);

        if let Some(filter) = selective
          && var_name != filter
        {
          continue;
        }

        let dst_ty_id = *ty_id;

        vars.push(ExportedVar {
          name: *name,
          ty_id: dst_ty_id,
          init: *init,
        });
      }

      Insn::EnumDef {
        name,
        variants,
        pubness,
        ..
      } => {
        if *pubness != Pubness::Yes {
          continue;
        }

        let enum_name = interner.get(*name);

        if let Some(filter) = selective
          && enum_name != filter
        {
          continue;
        }

        // Translate field types into the caller's type-checker.
        // The importing executor will re-intern the full enum
        // from this raw data so the EnumTyId lives in its
        // own table. Variant names are already shared via
        // the common interner.
        let dst_variants: Vec<(Symbol, u32, Vec<TyId>)> = variants
          .iter()
          .map(|(vname, disc, fields)| {
            let dst_fields: Vec<TyId> = fields.to_vec();

            (*vname, *disc, dst_fields)
          })
          .collect();

        enums.push(ExportedEnum {
          name: *name,
          variants: dst_variants,
        });
      }
      Insn::StructDef {
        name,
        ty_id,
        fields,
        pubness,
      } => {
        if *pubness != Pubness::Yes {
          continue;
        }

        let struct_name = interner.get(*name);

        if let Some(filter) = selective
          && struct_name != filter
        {
          continue;
        }

        structs.push(ExportedStruct {
          name: *name,
          ty_id: *ty_id,
          fields: fields.clone(),
        });
      }

      Insn::ConstDef {
        name,
        ty_id,
        value,
        pubness,
      } => {
        if *pubness != Pubness::Yes {
          continue;
        }

        let const_name = interner.get(*name);

        if let Some(filter) = selective
          && const_name != filter
        {
          continue;
        }

        consts.push(ExportedConst {
          name: *name,
          ty_id: *ty_id,
          value: *value,
        });
      }

      _ => {}
    }
  }

  // Filter out PackDecl and EnumDef — PackDecl is a namespace
  // directive with no codegen. EnumDef is handled by the
  // executor via `with_imports` and its ty_ids reference the
  // module's throwaway type checker; leaving them in the merged
  // SIR causes ty_id collisions in the codegen's enum_metas
  // HashMap.
  let sir_instructions = sir
    .instructions
    .into_iter()
    .filter(|i| !matches!(i, Insn::PackDecl { .. } | Insn::EnumDef { .. }))
    .collect();

  ModuleExports {
    funs,
    vars,
    enums,
    structs,
    consts,
    sir_instructions,
    next_value_id: sir.next_value_id,
    next_label_id: sir.next_label_id,
  }
}
