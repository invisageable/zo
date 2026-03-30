use zo_sir::{Insn, Sir};
use zo_ty::TyId;
use zo_value::{FunctionKind, Pubness, ValueId};

/// Build a SIR from a list of instructions.
pub fn make_sir(instructions: Vec<Insn>) -> Sir {
  let next_value_id = instructions.len() as u32;

  Sir {
    instructions,
    next_value_id,
    next_label_id: 0,
  }
}

/// Helper to build a simple function (FunDef + body + Return).
pub fn fun(
  name: zo_interner::Symbol,
  pubness: Pubness,
  body: Vec<Insn>,
) -> Vec<Insn> {
  let mut insns = Vec::with_capacity(body.len() + 2);

  insns.push(Insn::FunDef {
    name,
    params: vec![],
    return_ty: TyId(1),
    body_start: 0,
    kind: FunctionKind::UserDefined,
    pubness,
  });

  insns.extend(body);

  insns.push(Insn::Return {
    value: None,
    ty_id: TyId(1),
  });

  insns
}

/// Helper to build a Call instruction.
pub fn call(name: zo_interner::Symbol) -> Insn {
  Insn::Call {
    dst: ValueId(0),
    name,
    args: vec![],
    ty_id: TyId(1),
  }
}

/// Extract function names from SIR.
pub fn fun_names(sir: &Sir) -> Vec<zo_interner::Symbol> {
  sir
    .instructions
    .iter()
    .filter_map(|i| {
      if let Insn::FunDef { name, .. } = i {
        Some(*name)
      } else {
        None
      }
    })
    .collect()
}
