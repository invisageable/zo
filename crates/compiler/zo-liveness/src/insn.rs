//! SIR instruction introspection — defs and uses extraction.

use zo_interner::Symbol;
use zo_sir::{Insn, LoadSource};
use zo_value::ValueId;

/// Compute the `ValueId` produced by each SIR instruction.
///
/// Every value-producing instruction carries an explicit
/// `dst: ValueId`. Non-value instructions return `None`.
pub fn compute_value_ids(insns: &[Insn]) -> Vec<Option<ValueId>> {
  insns
    .iter()
    .map(|insn| match insn {
      Insn::ConstInt { dst, .. }
      | Insn::ConstFloat { dst, .. }
      | Insn::ConstBool { dst, .. }
      | Insn::ConstString { dst, .. }
      | Insn::Call { dst, .. }
      | Insn::Load { dst, .. }
      | Insn::BinOp { dst, .. }
      | Insn::UnOp { dst, .. }
      | Insn::ArrayLiteral { dst, .. }
      | Insn::ArrayIndex { dst, .. }
      | Insn::ArrayLen { dst, .. }
      | Insn::TupleLiteral { dst, .. }
      | Insn::TupleIndex { dst, .. }
      | Insn::EnumConstruct { dst, .. }
      | Insn::StructConstruct { dst, .. } => Some(*dst),
      Insn::Template { id, .. } => Some(*id),
      _ => None,
    })
    .collect()
}

/// Extract the `ValueId`s read by an instruction (uses).
pub fn insn_uses(insn: &Insn) -> Vec<ValueId> {
  match insn {
    Insn::BinOp { lhs, rhs, .. } => vec![*lhs, *rhs],
    Insn::Return { value: Some(v), .. } => vec![*v],
    Insn::Store { value, .. } => vec![*value],
    Insn::Call { args, .. } => args.clone(),
    Insn::UnOp { rhs, .. } => vec![*rhs],
    Insn::BranchIfNot { cond, .. } => vec![*cond],
    Insn::Directive { value, .. } => vec![*value],
    Insn::VarDef { init: Some(v), .. } => vec![*v],
    Insn::ArrayLiteral { elements, .. } => elements.clone(),
    Insn::ArrayIndex { array, index, .. } => {
      vec![*array, *index]
    }
    Insn::TupleIndex { tuple, .. } => vec![*tuple],
    Insn::FieldStore { base, value, .. } => {
      vec![*base, *value]
    }
    Insn::ArrayStore {
      array,
      index,
      value,
      ..
    } => {
      vec![*array, *index, *value]
    }
    Insn::ArrayLen { array, .. } => vec![*array],
    Insn::StructConstruct { fields, .. } => fields.clone(),
    Insn::EnumConstruct { fields, .. } => fields.clone(),
    Insn::TupleLiteral { elements, .. } => elements.clone(),
    _ => vec![],
  }
}

/// Extract the named variable defined by a `Store` instruction.
pub fn insn_var_def(insn: &Insn) -> Option<Symbol> {
  match insn {
    Insn::Store { name, .. } => Some(*name),
    _ => None,
  }
}

/// Extract the named variable used by a `Load { Local }`.
pub fn insn_var_use(insn: &Insn) -> Option<Symbol> {
  match insn {
    Insn::Load {
      src: LoadSource::Local(name),
      ..
    } => Some(*name),
    _ => None,
  }
}
