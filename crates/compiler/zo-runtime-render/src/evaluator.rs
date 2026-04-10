//! Minimal SIR evaluator for reactive event handler bodies.
//!
//! Executes closure SIR instructions at runtime when events
//! fire. Maps captured parameters to `StateCell`s so mutations
//! are visible to the template re-render.

use crate::render::{StateCell, StateValue};

use zo_interner::Symbol;
use zo_sir::{BinOp, Insn, LoadSource};
use zo_value::{FunctionKind, ValueId};

use rustc_hash::FxHashMap as HashMap;

/// Runtime value during evaluation.
#[derive(Clone, Debug)]
enum Val {
  Int(i64),
  Float(f64),
  Bool(bool),
  Str(String),
  Unit,
}

impl Val {
  fn to_state_value(&self) -> StateValue {
    match self {
      Val::Int(n) => StateValue::Int(*n),
      Val::Float(f) => StateValue::Float(*f),
      Val::Bool(b) => StateValue::Bool(*b),
      Val::Str(s) => StateValue::Str(s.clone()),
      Val::Unit => StateValue::Int(0),
    }
  }

  fn from_state_value(sv: &StateValue) -> Self {
    match sv {
      StateValue::Int(n) => Val::Int(*n),
      StateValue::Float(f) => Val::Float(*f),
      StateValue::Bool(b) => Val::Bool(*b),
      StateValue::Str(s) => Val::Str(s.clone()),
    }
  }
}

/// Evaluates a closure's SIR body with access to shared state.
pub struct HandlerEvaluator {
  /// SSA value registers: ValueId → Val.
  regs: HashMap<u32, Val>,
}

impl HandlerEvaluator {
  pub fn new() -> Self {
    Self {
      regs: HashMap::default(),
    }
  }

  /// Execute a closure handler body from SIR instructions.
  ///
  /// - `instructions`: the full SIR instruction stream.
  /// - `closure_name`: the Symbol of the closure to execute.
  /// - `state`: state cells for captured mut variables.
  /// - `capture_map`: maps param index → state cell index.
  pub fn execute(
    &mut self,
    instructions: &[Insn],
    closure_name: Symbol,
    state: &[StateCell],
    capture_map: &[(usize, usize)],
  ) {
    self.regs.clear();

    // Find the closure's FunDef and its body range.
    let mut body_start = None;
    let mut params: Vec<(Symbol, usize)> = Vec::new();

    for (i, insn) in instructions.iter().enumerate() {
      if let Insn::FunDef {
        name,
        params: fn_params,
        kind: FunctionKind::Closure { capture_count },
        ..
      } = insn
        && *name == closure_name
      {
        body_start = Some(i + 1);

        // Map captured params to state cells.
        let cc = *capture_count as usize;

        for (pi, (sym, _)) in fn_params.iter().enumerate().take(cc) {
          if let Some(&(_, slot_idx)) =
            capture_map.iter().find(|(ci, _)| *ci == pi)
          {
            let val = Val::from_state_value(&state[slot_idx].get());

            self.regs.insert(pi as u32 | 0x8000_0000, val);
            params.push((*sym, slot_idx));
          }
        }

        break;
      }
    }

    let Some(start) = body_start else {
      return;
    };

    // Execute instructions until Return.
    for insn in &instructions[start..] {
      match insn {
        Insn::ConstInt { dst, value, .. } => {
          self.regs.insert(dst.0, Val::Int(*value as i64));
        }

        Insn::ConstFloat { dst, value, .. } => {
          self.regs.insert(dst.0, Val::Float(*value));
        }

        Insn::ConstBool { dst, value, .. } => {
          self.regs.insert(dst.0, Val::Bool(*value));
        }

        Insn::Load { dst, src, .. } => {
          let val = match src {
            LoadSource::Param(idx) => self
              .regs
              .get(&(*idx | 0x8000_0000))
              .cloned()
              .unwrap_or(Val::Unit),
            LoadSource::Local(sym) => {
              // Look up from state cells by name.
              params
                .iter()
                .find(|(s, _)| s == sym)
                .map(|(_, slot_idx)| {
                  Val::from_state_value(&state[*slot_idx].get())
                })
                .unwrap_or(Val::Unit)
            }
          };

          self.regs.insert(dst.0, val);
        }

        Insn::BinOp {
          dst, op, lhs, rhs, ..
        } => {
          let l = self.get(lhs);
          let r = self.get(rhs);

          let result = self.eval_binop(op, &l, &r);

          self.regs.insert(dst.0, result);
        }

        Insn::UnOp { dst, op, rhs, .. } => {
          let r = self.get(rhs);

          let result = self.eval_unop(op, &r);

          self.regs.insert(dst.0, result);
        }

        Insn::Store { name, value, .. } => {
          let val = self.get(value);

          // Write back to the state cell.
          if let Some((_, slot_idx)) = params.iter().find(|(s, _)| s == name) {
            state[*slot_idx].set(val.to_state_value());
          }

          // Also update param register for subsequent reads.
          if let Some((pi, _)) =
            params.iter().enumerate().find(|(_, (s, _))| s == name)
          {
            self.regs.insert(pi as u32 | 0x8000_0000, val);
          }
        }

        Insn::Return { .. } => break,

        // Skip other instructions (FunDef of nested
        // closures, Nop, etc.)
        _ => {}
      }
    }
  }

  fn get(&self, id: &ValueId) -> Val {
    self.regs.get(&id.0).cloned().unwrap_or(Val::Unit)
  }

  fn eval_binop(&self, op: &BinOp, l: &Val, r: &Val) -> Val {
    match (l, r) {
      (Val::Int(a), Val::Int(b)) => Val::Int(match op {
        BinOp::Add => a + b,
        BinOp::Sub => a - b,
        BinOp::Mul => a * b,
        BinOp::Div if *b != 0 => a / b,
        BinOp::Rem if *b != 0 => a % b,
        BinOp::Eq => return Val::Bool(a == b),
        BinOp::Neq => return Val::Bool(a != b),
        BinOp::Lt => return Val::Bool(a < b),
        BinOp::Lte => return Val::Bool(a <= b),
        BinOp::Gt => return Val::Bool(a > b),
        BinOp::Gte => return Val::Bool(a >= b),
        BinOp::BitAnd => a & b,
        BinOp::BitOr => a | b,
        BinOp::BitXor => a ^ b,
        BinOp::Shl => a << b,
        BinOp::Shr => a >> b,
        _ => 0,
      }),
      (Val::Float(a), Val::Float(b)) => Val::Float(match op {
        BinOp::Add => a + b,
        BinOp::Sub => a - b,
        BinOp::Mul => a * b,
        BinOp::Div if *b != 0.0 => a / b,
        BinOp::Eq => return Val::Bool(a == b),
        BinOp::Neq => return Val::Bool(a != b),
        BinOp::Lt => return Val::Bool(a < b),
        BinOp::Lte => return Val::Bool(a <= b),
        BinOp::Gt => return Val::Bool(a > b),
        BinOp::Gte => return Val::Bool(a >= b),
        _ => 0.0,
      }),
      (Val::Bool(a), Val::Bool(b)) => Val::Bool(match op {
        BinOp::And => *a && *b,
        BinOp::Or => *a || *b,
        BinOp::Eq => a == b,
        BinOp::Neq => a != b,
        _ => false,
      }),
      _ => Val::Unit,
    }
  }

  fn eval_unop(&self, op: &zo_sir::UnOp, r: &Val) -> Val {
    match (op, r) {
      (zo_sir::UnOp::Neg, Val::Int(n)) => Val::Int(-n),
      (zo_sir::UnOp::Neg, Val::Float(f)) => Val::Float(-f),
      (zo_sir::UnOp::Not, Val::Bool(b)) => Val::Bool(!b),
      _ => Val::Unit,
    }
  }
}

impl Default for HandlerEvaluator {
  fn default() -> Self {
    Self::new()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use zo_sir::Sir;
  use zo_ty::TyId;
  use zo_value::Pubness;

  fn int_ty() -> TyId {
    TyId(3)
  }

  fn make_counter_closure(
    sir: &mut Sir,
    interner: &mut zo_interner::Interner,
    delta: i64,
  ) -> Symbol {
    let name = interner.intern(if delta > 0 {
      "__closure_inc"
    } else {
      "__closure_dec"
    });
    let count_sym = interner.intern("count");

    // FunDef: closure with 1 capture (count).
    sir.emit(Insn::FunDef {
      name,
      params: vec![(count_sym, int_ty())],
      return_ty: int_ty(),
      body_start: 1,
      kind: FunctionKind::Closure { capture_count: 1 },
      pubness: Pubness::No,
    });

    // Load captured count: Load Param(0).
    let load_dst = ValueId(sir.next_value_id);

    sir.next_value_id += 1;

    sir.emit(Insn::Load {
      dst: load_dst,
      src: LoadSource::Param(0),
      ty_id: int_ty(),
    });

    // ConstInt: the delta value.
    let const_dst = ValueId(sir.next_value_id);

    sir.next_value_id += 1;

    sir.emit(Insn::ConstInt {
      dst: const_dst,
      value: delta.unsigned_abs(),
      ty_id: int_ty(),
    });

    // BinOp: count + delta or count - delta.
    let binop_dst = ValueId(sir.next_value_id);

    sir.next_value_id += 1;

    let op = if delta > 0 { BinOp::Add } else { BinOp::Sub };

    sir.emit(Insn::BinOp {
      dst: binop_dst,
      op,
      lhs: load_dst,
      rhs: const_dst,
      ty_id: int_ty(),
    });

    // Store: write back to count.
    sir.emit(Insn::Store {
      name: count_sym,
      value: binop_dst,
      ty_id: int_ty(),
    });

    // Return.
    sir.emit(Insn::Return {
      value: None,
      ty_id: int_ty(),
    });

    name
  }

  #[test]
  fn test_evaluate_add_assign() {
    let mut sir = Sir::new();
    let mut interner = zo_interner::Interner::new();

    let name = make_counter_closure(&mut sir, &mut interner, 1);

    let state = vec![StateCell::new(StateValue::Int(5))];
    let capture_map = vec![(0, 0)]; // param 0 → slot 0

    let mut eval = HandlerEvaluator::new();

    eval.execute(&sir.instructions, name, &state, &capture_map);

    assert_eq!(state[0].get(), StateValue::Int(6));
  }

  #[test]
  fn test_evaluate_sub_assign() {
    let mut sir = Sir::new();
    let mut interner = zo_interner::Interner::new();

    let name = make_counter_closure(&mut sir, &mut interner, -1);

    let state = vec![StateCell::new(StateValue::Int(5))];
    let capture_map = vec![(0, 0)];

    let mut eval = HandlerEvaluator::new();

    eval.execute(&sir.instructions, name, &state, &capture_map);

    assert_eq!(state[0].get(), StateValue::Int(4));
  }

  #[test]
  fn test_evaluate_from_zero() {
    let mut sir = Sir::new();
    let mut interner = zo_interner::Interner::new();

    let name = make_counter_closure(&mut sir, &mut interner, 1);

    let state = vec![StateCell::new(StateValue::Int(0))];
    let capture_map = vec![(0, 0)];

    let mut eval = HandlerEvaluator::new();

    // Click 3 times.
    for _ in 0..3 {
      eval.execute(&sir.instructions, name, &state, &capture_map);
    }

    assert_eq!(state[0].get(), StateValue::Int(3));
  }

  #[test]
  fn test_evaluate_negative() {
    let mut sir = Sir::new();
    let mut interner = zo_interner::Interner::new();

    let name = make_counter_closure(&mut sir, &mut interner, -1);

    let state = vec![StateCell::new(StateValue::Int(0))];
    let capture_map = vec![(0, 0)];

    let mut eval = HandlerEvaluator::new();

    eval.execute(&sir.instructions, name, &state, &capture_map);
    eval.execute(&sir.instructions, name, &state, &capture_map);

    assert_eq!(state[0].get(), StateValue::Int(-2));
  }

  #[test]
  fn test_evaluate_unknown_closure_noop() {
    let sir = Sir::new();
    let _interner = zo_interner::Interner::new();

    let state = vec![StateCell::new(StateValue::Int(42))];

    let mut eval = HandlerEvaluator::new();

    // Non-existent closure — should do nothing.
    eval.execute(&sir.instructions, Symbol::new(9999), &state, &[]);

    assert_eq!(state[0].get(), StateValue::Int(42));
  }
}
