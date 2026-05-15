//! Minimal SIR evaluator for reactive event handler bodies.
//!
//! Executes closure SIR instructions at runtime when events
//! fire. Maps captured parameters to `StateCell`s so mutations
//! are visible to the template re-render.

use crate::render::{EventPayload, StateCell, StateValue};

use zo_interner::Symbol;
use zo_sir::{BinOp, Insn, LoadSource};
use zo_value::{FunctionKind, ValueId};

use rustc_hash::FxHashMap as HashMap;

/// Runtime value during evaluation.
#[derive(Clone, Debug)]
pub enum Val {
  Int(i64),
  Float(f64),
  Bool(bool),
  Str(String),
  /// Sentinel marking a `Load` of the event-payload param.
  /// The actual payload lives on the `execute` call frame
  /// (`event: Option<&EventPayload>`); a downstream
  /// `TupleIndex` reads it directly. Carrying it as a
  /// unit variant avoids cloning the payload's `String`
  /// once per `Load` and once per field projection.
  Event,
  /// Reference to a state-cell-backed `[]str` capture. The
  /// `usize` is the slot index into the evaluator's
  /// `state` slice. `Insn::ArrayPush` against this Val
  /// mutates the underlying `StateCell` so the post-event
  /// list-binding re-render sees the updated array.
  StrArrRef(usize),
  Unit,
}

impl Val {
  fn to_state_value(&self) -> StateValue {
    match self {
      Val::Int(n) => StateValue::Int(*n),
      Val::Float(f) => StateValue::Float(*f),
      Val::Bool(b) => StateValue::Bool(*b),
      Val::Str(s) => StateValue::Str(s.clone()),
      // `Event` sentinel and the `StrArrRef` borrow can't
      // round-trip into a state cell — both reference
      // values that live on the eval frame, not in the
      // cell. Collapse to `Int(0)` so the slot stays
      // well-formed; misuse surfaces as a stale value
      // rather than a panic.
      Val::Event | Val::StrArrRef(_) | Val::Unit => StateValue::Int(0),
    }
  }

  fn from_state_value(sv: &StateValue) -> Self {
    match sv {
      StateValue::Int(n) => Val::Int(*n),
      StateValue::Float(f) => Val::Float(*f),
      StateValue::Bool(b) => Val::Bool(*b),
      StateValue::Str(s) => Val::Str(s.clone()),
      // `[]str` captures lift via `StrArrRef` not a direct
      // copy — the param's `Load` site materializes the
      // ref so a downstream `ArrayPush` can mutate the
      // underlying cell.
      StateValue::Strs(_) => Val::Unit,
    }
  }

  /// Renders any `Val` as the string a `UiCommand::Text`
  /// slot should display. Mirrors `StateValue::display`.
  pub fn display(&self) -> String {
    match self {
      Val::Int(n) => n.to_string(),
      Val::Float(f) => f.to_string(),
      Val::Bool(b) => b.to_string(),
      Val::Str(s) => s.clone(),
      // The sentinel renders empty — the only meaningful
      // display happens after a `TupleIndex` projection,
      // which materializes a `Val::Str` from the payload.
      Val::Event | Val::StrArrRef(_) | Val::Unit => String::new(),
    }
  }
}

/// Evaluates a closure's SIR body with access to shared state.
pub struct HandlerEvaluator {
  /// SSA value registers keyed by `ValueId.0`. ValueIds are
  /// minted by the SIR builder's monotonic counter.
  regs: HashMap<u32, Val>,
  /// Per-name local register file for ternary sink stores
  /// (`store __branch_result_0__, %X`) and any other
  /// non-captured locals. Keyed by `Symbol.as_u32()` —
  /// disjoint from `regs`'s ValueId namespace because
  /// ValueIds and Symbols are minted by independent
  /// counters and could collide otherwise.
  locals: HashMap<u32, Val>,
}

impl HandlerEvaluator {
  pub fn new() -> Self {
    Self {
      regs: HashMap::default(),
      locals: HashMap::default(),
    }
  }

  /// Execute a closure handler body from SIR instructions.
  ///
  /// - `instructions`: the full SIR instruction stream.
  /// - `closure_name`: the Symbol of the closure to execute.
  /// - `state`: state cells for captured mut variables.
  /// - `capture_map`: maps param index → state cell index.
  /// - `strings`: per-Symbol string snapshot used to
  ///   resolve `Insn::ConstString`. The driver builds this
  ///   from the interner once and clones an `Arc` into each
  ///   handler closure (`Interner` itself is not `Clone` due
  ///   to internal self-references).
  ///
  /// Returns the closure's `Insn::Return` value (if any).
  /// Click handlers ignore this; computed text bindings
  /// stamp the result into a `UiCommand::Text`.
  ///
  /// `event` carries the runtime-built payload for the firing
  /// event when the closure has an explicit user param bound
  /// to `@input`/`@change`. Computed-binding closures and
  /// `@click` handlers don't read the payload — they pass
  /// `None` (or `&EventPayload::default()` from the dispatch
  /// site, which is structurally equivalent for the field
  /// arms below since they bail to `Val::Unit` on absent
  /// fields).
  pub fn execute(
    &mut self,
    instructions: &[Insn],
    closure_name: Symbol,
    state: &[StateCell],
    capture_map: &[(usize, usize)],
    strings: &[String],
    event: Option<&EventPayload>,
  ) -> Option<Val> {
    self.regs.clear();
    self.locals.clear();

    // Find the closure's FunDef and bound the body to the
    // function's own SIR span — preventing the PC loop
    // from bleeding into the next FunDef when the body has
    // jumps that target labels outside its range (which
    // shouldn't happen, but a stray bug elsewhere would
    // otherwise crash the runtime).
    let mut body_start = None;
    let mut body_end = instructions.len();
    let mut params: Vec<(Symbol, usize)> = Vec::new();
    // First user-param index — captures occupy `[0, cc)`,
    // user params start at `cc`. The event payload, when an
    // event closure has one, is the first user param.
    let mut event_param_idx: Option<u32> = None;

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

        let cc = *capture_count as usize;

        for (pi, (sym, _)) in fn_params.iter().enumerate().take(cc) {
          if let Some(&(_, slot_idx)) =
            capture_map.iter().find(|(ci, _)| *ci == pi)
          {
            // `[]str` captures lift as a slot reference so
            // `Insn::ArrayPush` against the param mutates
            // the underlying state cell. Other values copy
            // by snapshotting the cell's current state. The
            // `is_strs` peek avoids cloning the `Vec<String>`
            // just to discard it on the non-`Strs` arm.
            let val = if state[slot_idx].is_strs() {
              Val::StrArrRef(slot_idx)
            } else {
              Val::from_state_value(&state[slot_idx].get())
            };

            self.regs.insert(pi as u32 | 0x8000_0000, val);
            params.push((*sym, slot_idx));
          }
        }

        if event.is_some() && fn_params.len() > cc {
          event_param_idx = Some(cc as u32);
        }

        for (j, next) in instructions.iter().enumerate().skip(i + 1) {
          if matches!(next, Insn::FunDef { .. }) {
            body_end = j;
            break;
          }
        }

        break;
      }
    }

    let start = body_start?;

    // Pre-index labels for jump resolution.
    let mut label_to_pc: HashMap<u32, usize> = HashMap::default();

    for (j, insn) in instructions[start..body_end].iter().enumerate() {
      if let Insn::Label { id } = insn {
        label_to_pc.insert(*id, start + j);
      }
    }

    let mut pc = start;
    let mut result: Option<Val> = None;

    while pc < body_end {
      match &instructions[pc] {
        Insn::ConstInt { dst, value, .. } => {
          self.regs.insert(dst.0, Val::Int(*value as i64));
        }

        Insn::ConstFloat { dst, value, .. } => {
          self.regs.insert(dst.0, Val::Float(*value));
        }

        Insn::ConstBool { dst, value, .. } => {
          self.regs.insert(dst.0, Val::Bool(*value));
        }

        Insn::ConstString { dst, symbol, .. } => {
          let s = strings.get(symbol.0 as usize).cloned().unwrap_or_default();

          self.regs.insert(dst.0, Val::Str(s));
        }

        Insn::Load { dst, src, .. } => {
          let val = match src {
            LoadSource::Param(idx) => {
              // The event-payload param is identified by
              // index, not by symbol — it's the first user
              // param of an event-bound closure. The sentinel
              // `Val::Event` flows into the next `TupleIndex`,
              // which reads the actual payload directly off
              // the `event` argument — no per-load cloning.
              if event_param_idx == Some(*idx) && event.is_some() {
                Val::Event
              } else {
                self
                  .regs
                  .get(&(*idx | 0x8000_0000))
                  .cloned()
                  .unwrap_or(Val::Unit)
              }
            }
            LoadSource::Local(sym) => {
              // Branch sinks and other transient locals
              // live in `self.locals`; captured mut vars
              // in state cells. Synthetic locals win on
              // collision so a sink store for
              // `__branch_result_0__` isn't shadowed by an
              // unrelated state cell.
              if let Some(v) = self.locals.get(&sym.as_u32()) {
                v.clone()
              } else {
                params
                  .iter()
                  .find(|(s, _)| s == sym)
                  .map(|(_, slot_idx)| {
                    Val::from_state_value(&state[*slot_idx].get())
                  })
                  .unwrap_or(Val::Unit)
              }
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

        Insn::TupleIndex {
          dst, tuple, index, ..
        } => {
          // Struct/tuple field projections lower to
          // `TupleIndex` in SIR. The only kind we resolve
          // here is the event payload — everything else is
          // either a regular tuple constant (not yet
          // materialized in the evaluator) or an
          // unsupported aggregate, in which case `Val::Unit`
          // is the safe fallback. Reading `event` directly
          // here (instead of via a cloned `Val::Event`) is
          // the only `String` clone per `e.value` read.
          let val = match (self.regs.get(&tuple.0), event) {
            (Some(Val::Event), Some(p)) if *index == 0 => {
              Val::Str(p.value.clone())
            }
            _ => Val::Unit,
          };

          self.regs.insert(dst.0, val);
        }

        Insn::Store { name, value, .. } => {
          let val = self.get(value);

          if let Some((_, slot_idx)) = params.iter().find(|(s, _)| s == name) {
            state[*slot_idx].set(val.to_state_value());

            if let Some((pi, _)) =
              params.iter().enumerate().find(|(_, (s, _))| s == name)
            {
              self.regs.insert(pi as u32 | 0x8000_0000, val);
            }
          } else {
            // Synthetic local — branch sink, etc.
            self.locals.insert(name.as_u32(), val);
          }
        }

        Insn::ArrayPush { array, value, .. } => {
          // Mutate the underlying state cell when the
          // array reg is a `StrArrRef` (a captured `mut
          // []str`). Other shapes (compile-time
          // construction, locals) aren't reachable from
          // event-handler bodies today, so silently no-op.
          if let Val::StrArrRef(slot_idx) = self.get(array) {
            let pushed = match self.get(value) {
              Val::Str(s) => s,
              other => other.display(),
            };

            state[slot_idx].mutate(|cell| {
              if let StateValue::Strs(items) = cell {
                items.push(pushed);
              }
            });
          }
        }

        Insn::Jump { target } => {
          if let Some(&dst_pc) = label_to_pc.get(target) {
            pc = dst_pc;
            continue;
          }
        }

        Insn::BranchIfNot { cond, target } => {
          let cond_val = self.get(cond);
          let take = matches!(cond_val, Val::Bool(false));

          if take && let Some(&dst_pc) = label_to_pc.get(target) {
            pc = dst_pc;
            continue;
          }
        }

        Insn::Label { .. } => {
          // No-op: jump targets are pre-resolved above.
        }

        Insn::Return { value, .. } => {
          if let Some(v) = value {
            result = Some(self.get(v));
          }

          break;
        }

        // Skip other instructions (FunDef of nested
        // closures, Nop, etc.)
        _ => {}
      }

      pc += 1;
    }

    result
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
      mut_self: false,
      link_name: None,
      owning_pack: None,
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

    eval.execute(&sir.instructions, name, &state, &capture_map, &[], None);

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

    eval.execute(&sir.instructions, name, &state, &capture_map, &[], None);

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
      eval.execute(&sir.instructions, name, &state, &capture_map, &[], None);
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

    eval.execute(&sir.instructions, name, &state, &capture_map, &[], None);
    eval.execute(&sir.instructions, name, &state, &capture_map, &[], None);

    assert_eq!(state[0].get(), StateValue::Int(-2));
  }

  #[test]
  fn test_evaluate_unknown_closure_noop() {
    let sir = Sir::new();

    let state = vec![StateCell::new(StateValue::Int(42))];

    let mut eval = HandlerEvaluator::new();

    // Non-existent closure — should do nothing.
    eval.execute(&sir.instructions, Symbol::new(9999), &state, &[], &[], None);

    assert_eq!(state[0].get(), StateValue::Int(42));
  }

  // === EVENT PAYLOAD ===

  /// Builds a closure shaped like `fn(e) => x = e.value` —
  /// one int capture (`x`), one explicit user param
  /// (`e: Event`), body reads `e.value` (lowered as
  /// `Load Param(1)` + `TupleIndex(_, 0)`) and stores the
  /// resulting `str` back into the captured slot. Mirrors
  /// what the executor emits for
  /// `@input={fn(e) => x = e.value}`.
  fn make_event_input_closure(
    sir: &mut Sir,
    interner: &mut zo_interner::Interner,
  ) -> Symbol {
    let name = interner.intern("__closure_input");
    let x_sym = interner.intern("x");
    let e_sym = interner.intern("e");

    sir.emit(Insn::FunDef {
      name,
      params: vec![(x_sym, str_ty()), (e_sym, str_ty())],
      return_ty: TyId(1),
      body_start: 1,
      kind: FunctionKind::Closure { capture_count: 1 },
      pubness: Pubness::No,
      mut_self: false,
      link_name: None,
      owning_pack: None,
    });

    let e_val = ValueId(sir.next_value_id);
    sir.next_value_id += 1;
    sir.emit(Insn::Load {
      dst: e_val,
      src: LoadSource::Param(1),
      ty_id: str_ty(),
    });

    let field = ValueId(sir.next_value_id);
    sir.next_value_id += 1;
    sir.emit(Insn::TupleIndex {
      dst: field,
      tuple: e_val,
      index: 0,
      ty_id: str_ty(),
    });

    sir.emit(Insn::Store {
      name: x_sym,
      value: field,
      ty_id: str_ty(),
    });

    sir.emit(Insn::Return {
      value: None,
      ty_id: TyId(1),
    });

    name
  }

  #[test]
  fn test_evaluate_event_value_propagates_to_capture() {
    let mut sir = Sir::new();
    let mut interner = zo_interner::Interner::new();

    let name = make_event_input_closure(&mut sir, &mut interner);
    let strings = interner.snapshot();

    let state = vec![StateCell::new(StateValue::Str(String::new()))];
    let capture_map = vec![(0, 0)];

    let payload = EventPayload {
      value: "hello world".to_string(),
    };

    let mut eval = HandlerEvaluator::new();
    eval.execute(
      &sir.instructions,
      name,
      &state,
      &capture_map,
      &strings,
      Some(&payload),
    );

    assert_eq!(state[0].get(), StateValue::Str("hello world".to_string()));
  }

  #[test]
  fn test_evaluate_event_none_falls_through_to_unit() {
    // Without a payload, the body's `Load Param(1)` resolves
    // to whatever's in regs — `Val::Unit` if absent. The
    // `TupleIndex` arm then yields `Val::Unit`, so the store
    // collapses to `StateValue::Int(0)`. The state slot
    // remains observable; nothing panics.
    let mut sir = Sir::new();
    let mut interner = zo_interner::Interner::new();

    let name = make_event_input_closure(&mut sir, &mut interner);
    let strings = interner.snapshot();

    let state = vec![StateCell::new(StateValue::Str("orig".to_string()))];
    let capture_map = vec![(0, 0)];

    let mut eval = HandlerEvaluator::new();
    eval.execute(
      &sir.instructions,
      name,
      &state,
      &capture_map,
      &strings,
      None,
    );

    // No panic; slot collapsed via `to_state_value()` from
    // `Val::Unit`.
    assert_eq!(state[0].get(), StateValue::Int(0));
  }

  // === COMPUTED BINDING SUPPORT — control flow + return ===

  fn str_ty() -> TyId {
    TyId(4)
  }

  fn bool_ty() -> TyId {
    TyId(5)
  }

  /// Builds a closure with a single int capture and the
  /// `when count == N ? "a" : "b"` ternary body, mirroring
  /// the shape the executor emits for compound `{when …}`
  /// template interpolations.
  fn make_when_closure(
    sir: &mut Sir,
    interner: &mut zo_interner::Interner,
    cmp_value: i64,
    arm_true: &str,
    arm_false: &str,
  ) -> (Symbol, Vec<String>) {
    let name = interner.intern("__interp_when");
    let count_sym = interner.intern("count");
    let sink_sym = interner.intern("__branch_result_0__");
    let true_sym = interner.intern(arm_true);
    let false_sym = interner.intern(arm_false);

    let else_label = sir.next_label();
    let end_label = sir.next_label();

    sir.emit(Insn::FunDef {
      name,
      params: vec![(count_sym, int_ty())],
      return_ty: str_ty(),
      body_start: 1,
      kind: FunctionKind::Closure { capture_count: 1 },
      pubness: Pubness::No,
      mut_self: false,
      link_name: None,
      owning_pack: None,
    });

    // Load count → cmp value → eq → BranchIfNot else.
    let load_dst = ValueId(sir.next_value_id);
    sir.next_value_id += 1;
    sir.emit(Insn::Load {
      dst: load_dst,
      src: LoadSource::Param(0),
      ty_id: int_ty(),
    });

    let cmp_dst = ValueId(sir.next_value_id);
    sir.next_value_id += 1;
    sir.emit(Insn::ConstInt {
      dst: cmp_dst,
      value: cmp_value as u64,
      ty_id: int_ty(),
    });

    let eq_dst = ValueId(sir.next_value_id);
    sir.next_value_id += 1;
    sir.emit(Insn::BinOp {
      dst: eq_dst,
      op: BinOp::Eq,
      lhs: load_dst,
      rhs: cmp_dst,
      ty_id: bool_ty(),
    });

    sir.emit(Insn::BranchIfNot {
      cond: eq_dst,
      target: else_label,
    });

    // True arm: ConstString → store sink → jump end.
    let true_dst = ValueId(sir.next_value_id);
    sir.next_value_id += 1;
    sir.emit(Insn::ConstString {
      dst: true_dst,
      symbol: true_sym,
      ty_id: str_ty(),
    });
    sir.emit(Insn::Store {
      name: sink_sym,
      value: true_dst,
      ty_id: str_ty(),
    });
    sir.emit(Insn::Jump { target: end_label });

    // Else arm: label → ConstString → store sink.
    sir.emit(Insn::Label { id: else_label });
    let false_dst = ValueId(sir.next_value_id);
    sir.next_value_id += 1;
    sir.emit(Insn::ConstString {
      dst: false_dst,
      symbol: false_sym,
      ty_id: str_ty(),
    });
    sir.emit(Insn::Store {
      name: sink_sym,
      value: false_dst,
      ty_id: str_ty(),
    });

    // Merge: end label → load sink → return.
    sir.emit(Insn::Label { id: end_label });
    let load_sink_dst = ValueId(sir.next_value_id);
    sir.next_value_id += 1;
    sir.emit(Insn::Load {
      dst: load_sink_dst,
      src: LoadSource::Local(sink_sym),
      ty_id: str_ty(),
    });

    sir.emit(Insn::Return {
      value: Some(load_sink_dst),
      ty_id: str_ty(),
    });

    let strings = interner.snapshot();

    (name, strings)
  }

  #[test]
  fn test_evaluate_const_string_returned() {
    let mut sir = Sir::new();
    let mut interner = zo_interner::Interner::new();

    let name = interner.intern("__interp_lit");
    sir.emit(Insn::FunDef {
      name,
      params: Vec::new(),
      return_ty: str_ty(),
      body_start: 1,
      kind: FunctionKind::Closure { capture_count: 0 },
      pubness: Pubness::No,
      mut_self: false,
      link_name: None,
      owning_pack: None,
    });

    let lit_sym = interner.intern("hello");
    let lit_dst = ValueId(sir.next_value_id);
    sir.next_value_id += 1;
    sir.emit(Insn::ConstString {
      dst: lit_dst,
      symbol: lit_sym,
      ty_id: str_ty(),
    });

    sir.emit(Insn::Return {
      value: Some(lit_dst),
      ty_id: str_ty(),
    });

    let strings = interner.snapshot();
    let mut eval = HandlerEvaluator::new();
    let result =
      eval.execute(&sir.instructions, name, &[], &[], &strings, None);

    match result {
      Some(Val::Str(s)) => assert_eq!(s, "hello"),
      other => panic!("expected Some(Val::Str(\"hello\")), got {other:?}"),
    }
  }

  #[test]
  fn test_evaluate_when_ternary_true_branch() {
    let mut sir = Sir::new();
    let mut interner = zo_interner::Interner::new();

    let (name, strings) =
      make_when_closure(&mut sir, &mut interner, 1, "time", "times");

    // count == 1 → BranchIfNot (false) — fall through to
    // true arm. Result must be "time".
    let state = vec![StateCell::new(StateValue::Int(1))];
    let capture_map = vec![(0, 0)];

    let mut eval = HandlerEvaluator::new();
    let result = eval.execute(
      &sir.instructions,
      name,
      &state,
      &capture_map,
      &strings,
      None,
    );

    match result {
      Some(Val::Str(s)) => assert_eq!(s, "time"),
      other => panic!("expected \"time\", got {other:?}"),
    }
  }

  #[test]
  fn test_evaluate_when_ternary_false_branch() {
    let mut sir = Sir::new();
    let mut interner = zo_interner::Interner::new();

    let (name, strings) =
      make_when_closure(&mut sir, &mut interner, 1, "time", "times");

    // count == 0 → BranchIfNot (true) — jump to else
    // arm. Result must be "times".
    let state = vec![StateCell::new(StateValue::Int(0))];
    let capture_map = vec![(0, 0)];

    let mut eval = HandlerEvaluator::new();
    let result = eval.execute(
      &sir.instructions,
      name,
      &state,
      &capture_map,
      &strings,
      None,
    );

    match result {
      Some(Val::Str(s)) => assert_eq!(s, "times"),
      other => panic!("expected \"times\", got {other:?}"),
    }
  }

  #[test]
  fn test_evaluate_jump_skips_dead_arm() {
    // Same closure, run twice with both states — the
    // implementation must NOT leak the previous run's sink
    // into the next via stale `self.locals`.
    let mut sir = Sir::new();
    let mut interner = zo_interner::Interner::new();

    let (name, strings) =
      make_when_closure(&mut sir, &mut interner, 1, "time", "times");

    let state = vec![StateCell::new(StateValue::Int(1))];
    let capture_map = vec![(0, 0)];

    let mut eval = HandlerEvaluator::new();

    let first = eval.execute(
      &sir.instructions,
      name,
      &state,
      &capture_map,
      &strings,
      None,
    );
    assert!(matches!(first, Some(Val::Str(ref s)) if s == "time"));

    state[0].set(StateValue::Int(0));
    let second = eval.execute(
      &sir.instructions,
      name,
      &state,
      &capture_map,
      &strings,
      None,
    );
    assert!(matches!(second, Some(Val::Str(ref s)) if s == "times"));
  }

  #[test]
  fn test_evaluate_return_none_for_void() {
    // Click handlers (like the existing counter closures)
    // emit `Return { value: None }`. The new return-value
    // path must surface `None` rather than fabricating a
    // value.
    let mut sir = Sir::new();
    let mut interner = zo_interner::Interner::new();

    let name = make_counter_closure(&mut sir, &mut interner, 1);

    let state = vec![StateCell::new(StateValue::Int(0))];
    let capture_map = vec![(0, 0)];

    let mut eval = HandlerEvaluator::new();
    let result =
      eval.execute(&sir.instructions, name, &state, &capture_map, &[], None);

    assert!(result.is_none());
  }

  #[test]
  fn test_evaluate_label_is_pure_marker() {
    // A bare Label between ConstInt and Return must not
    // affect the returned value or the program counter
    // beyond being skipped.
    let mut sir = Sir::new();
    let mut interner = zo_interner::Interner::new();

    let name = interner.intern("__interp_label_only");
    sir.emit(Insn::FunDef {
      name,
      params: Vec::new(),
      return_ty: int_ty(),
      body_start: 1,
      kind: FunctionKind::Closure { capture_count: 0 },
      pubness: Pubness::No,
      mut_self: false,
      link_name: None,
      owning_pack: None,
    });

    let some_label = sir.next_label();

    let dst = ValueId(sir.next_value_id);
    sir.next_value_id += 1;
    sir.emit(Insn::ConstInt {
      dst,
      value: 7,
      ty_id: int_ty(),
    });

    sir.emit(Insn::Label { id: some_label });

    sir.emit(Insn::Return {
      value: Some(dst),
      ty_id: int_ty(),
    });

    let strings = interner.snapshot();
    let mut eval = HandlerEvaluator::new();
    let result =
      eval.execute(&sir.instructions, name, &[], &[], &strings, None);

    match result {
      Some(Val::Int(7)) => {}
      other => panic!("expected Int(7), got {other:?}"),
    }
  }

  #[test]
  fn test_evaluate_body_bounded_by_next_fundef() {
    // Two FunDefs in the same SIR. Running the first must
    // stop at the second's FunDef boundary, even when the
    // first lacks a Return — otherwise the evaluator would
    // bleed into unrelated SIR.
    let mut sir = Sir::new();
    let mut interner = zo_interner::Interner::new();

    let name_a = interner.intern("__interp_a");
    let name_b = interner.intern("__interp_b");

    sir.emit(Insn::FunDef {
      name: name_a,
      params: Vec::new(),
      return_ty: int_ty(),
      body_start: 1,
      kind: FunctionKind::Closure { capture_count: 0 },
      pubness: Pubness::No,
      mut_self: false,
      link_name: None,
      owning_pack: None,
    });

    let dst_a = ValueId(sir.next_value_id);
    sir.next_value_id += 1;
    sir.emit(Insn::ConstInt {
      dst: dst_a,
      value: 1,
      ty_id: int_ty(),
    });
    // No Return — falls through.

    sir.emit(Insn::FunDef {
      name: name_b,
      params: Vec::new(),
      return_ty: int_ty(),
      body_start: 1,
      kind: FunctionKind::Closure { capture_count: 0 },
      pubness: Pubness::No,
      mut_self: false,
      link_name: None,
      owning_pack: None,
    });

    let dst_b = ValueId(sir.next_value_id);
    sir.next_value_id += 1;
    sir.emit(Insn::ConstInt {
      dst: dst_b,
      value: 999,
      ty_id: int_ty(),
    });
    sir.emit(Insn::Return {
      value: Some(dst_b),
      ty_id: int_ty(),
    });

    let strings = interner.snapshot();
    let mut eval = HandlerEvaluator::new();
    let result =
      eval.execute(&sir.instructions, name_a, &[], &[], &strings, None);

    // No Return inside name_a's body → result is None.
    // Critically, we must NOT see Int(999) leaked from
    // name_b.
    assert!(result.is_none());
  }
}
