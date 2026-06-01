//! SIR instruction introspection — defs and uses extraction.

use zo_interner::Symbol;
use zo_sir::{Insn, LoadSource};
use zo_value::ValueId;

/// The `ValueId` defined by a single SIR instruction, or
/// `None` for instructions that produce no value.
///
/// Every value-producing instruction carries an explicit
/// `dst: ValueId`. `SelectWait` exposes its arm index
/// (`out_which`); `Template` exposes its `id`. Single source
/// of truth for "what does this instruction define" — shared
/// by [`compute_value_ids`] and the def-site index.
#[inline]
pub fn insn_def(insn: &Insn) -> Option<ValueId> {
  match insn {
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
    | Insn::ArrayPop { dst, .. }
    | Insn::TupleLiteral { dst, .. }
    | Insn::TupleIndex { dst, .. }
    | Insn::EnumConstruct { dst, .. }
    | Insn::StructConstruct { dst, .. }
    | Insn::Cast { dst, .. }
    // Concurrency value-producing insns.
    | Insn::ChannelCreate { dst, .. }
    | Insn::ChannelRecv { dst, .. }
    | Insn::FnAddr { dst, .. }
    | Insn::TaskSpawn { dst, .. }
    | Insn::TaskAwait { dst, .. }
    | Insn::SelectRecv { dst, .. }
    | Insn::TaskCancelled { dst, .. }
    | Insn::StrSlice { dst, .. }
    | Insn::ToStr { dst, .. }
    | Insn::StringFormat { dst, .. } => Some(*dst),
    // `SelectWait` has two outputs (`out_which` +
    // companion `SelectRecv.dst` for the value).
    // Liveness tracks the arm index here; the value
    // register is defined by the paired `SelectRecv`.
    Insn::SelectWait { out_which, .. } => Some(*out_which),
    Insn::Template { id, .. } => Some(*id),
    _ => None,
  }
}

/// Compute the `ValueId` produced by each SIR instruction.
pub fn compute_value_ids(insns: &[Insn]) -> Vec<Option<ValueId>> {
  insns.iter().map(insn_def).collect()
}

/// Visit every `ValueId` read by `insn` (its uses),
/// calling `f` once per use in source order.
///
/// Push-based on purpose — replaces an allocating
/// `-> Vec<ValueId>` returning form that allocated a fresh
/// heap vector for every instruction in the liveness and
/// regalloc forward passes. Variable-length arms
/// (`Call.args`, `ArrayLiteral.elements`, ...) iterate
/// the existing `Vec` in the SIR; fixed-arity arms emit
/// directly.
pub fn visit_uses(insn: &Insn, mut f: impl FnMut(ValueId)) {
  match insn {
    Insn::BinOp { lhs, rhs, .. } => {
      f(*lhs);
      f(*rhs);
    }
    Insn::Return { value: Some(v), .. } => f(*v),
    Insn::Store { value, .. } => f(*value),
    Insn::Call { args, .. } => {
      for &v in args {
        f(v);
      }
    }
    Insn::UnOp { rhs, .. } => f(*rhs),
    Insn::BranchIfNot { cond, .. } => f(*cond),
    Insn::Directive { value, .. } => f(*value),
    Insn::VarDef { init: Some(v), .. } => f(*v),
    Insn::ArrayLiteral { elements, .. } => {
      for &v in elements {
        f(v);
      }
    }
    Insn::ArrayIndex { array, index, .. } => {
      f(*array);
      f(*index);
    }
    Insn::TupleIndex { tuple, .. } => f(*tuple),
    Insn::FieldStore { base, value, .. } => {
      f(*base);
      f(*value);
    }
    Insn::ArrayStore {
      array,
      index,
      value,
      ..
    } => {
      f(*array);
      f(*index);
      f(*value);
    }
    Insn::ArrayLen { array, .. } => f(*array),
    Insn::ArrayPush { array, value, .. } => {
      f(*array);
      f(*value);
    }
    Insn::ArrayPop { array, .. } => f(*array),
    Insn::StructConstruct { fields, .. } => {
      for &v in fields {
        f(v);
      }
    }
    Insn::EnumConstruct { fields, .. } => {
      for &v in fields {
        f(v);
      }
    }
    Insn::TupleLiteral { elements, .. } => {
      for &v in elements {
        f(v);
      }
    }
    Insn::Cast { src, .. } => f(*src),
    // Concurrency insns — enumerate their ValueId
    // operands so liveness keeps the defining insns
    // (TupleIndex / Load / etc.) alive through DCE.
    Insn::ChannelSend { channel, value, .. } => {
      f(*channel);
      f(*value);
    }
    Insn::ChannelRecv { channel, .. } => f(*channel),
    Insn::ChannelClose { channel } => f(*channel),
    Insn::TaskSpawn { args, .. } => {
      for &v in args {
        f(v);
      }
    }
    Insn::TaskAwait { task, .. } => f(*task),
    Insn::SelectWait { chans, .. } => {
      for &v in chans {
        f(v);
      }
    }
    // `SelectRecv` anchors liveness to its paired
    // `SelectWait`'s `out_which` so DCE can't reorder
    // or drop the wait.
    Insn::SelectRecv { which, .. } => f(*which),
    Insn::TaskCancelled { task, .. } => f(*task),
    Insn::TaskCancel { task } => f(*task),
    Insn::StrSlice { src, lo, hi, .. } => {
      f(*src);
      f(*lo);
      f(*hi);
    }
    Insn::ToStr { src, .. } => f(*src),
    Insn::StringFormat { segments, .. } => {
      for seg in segments {
        f(*seg);
      }
    }
    // `any <Abstract>` boxing: the heap-box copies bytes
    // from `src`, so the defining instruction must stay
    // live across the coercion. Without enumerating
    // `src` here, DCE classifies the source Load (or
    // construct) as unused and elides it.
    Insn::CoerceToDyn { src, .. } => f(*src),
    // Dynamic dispatch reads the receiver's fat-pointer
    // (two LDRs in the lowering) and each explicit arg
    // passes through a register move.
    Insn::DynDispatch { recv, args, .. } => {
      f(*recv);
      for &v in args {
        f(v);
      }
    }
    _ => {}
  }
}

/// Extract the named variable defined by a `Store` instruction.
pub fn insn_var_def(insn: &Insn) -> Option<Symbol> {
  match insn {
    Insn::Store { name, .. } => Some(*name),
    _ => None,
  }
}

/// Extract the named variable used by a `Load { Local }` or
/// a `Drop` (the drop is the binding's final use, so it must
/// keep the local live through DCE).
pub fn insn_var_use(insn: &Insn) -> Option<Symbol> {
  match insn {
    Insn::Load {
      src: LoadSource::Local(name),
      ..
    } => Some(*name),
    Insn::Drop { local, .. } => Some(*local),
    _ => None,
  }
}
