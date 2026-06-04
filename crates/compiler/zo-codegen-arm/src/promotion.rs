//! Register promotion (pragmatic mem2reg) for scalar
//! `mut`/`imu` locals.
//!
//! zo keeps every local in a stack slot: each read is an
//! `ldr`, each write an `str`. On register-pressure loops
//! that per-iteration memory traffic is the whole gap to an
//! optimizing C compiler. This pass lifts the hottest
//! register-sized scalar locals into dedicated callee-saved
//! registers (x19..x28) for the lifetime of a function, so
//! their reads and writes become register moves with no
//! memory access.
//!
//! No phi nodes are needed: a promoted symbol maps to ONE
//! physical register on every path through the function, so
//! control-flow merges resolve to the same register by
//! construction. The choice of callee-saved registers means
//! a promoted value survives a `bl` for free — the ABI
//! preserves x19..x28 — so there is no per-call spill.
//!
//! Scope: general-purpose scalars only (`int`, `s8..s64`,
//! `u8..u64`, `bool`, `char`, pointers). Floats (which would
//! use d8..d15) are a follow-up.

use zo_emitter_arm::Register;
use zo_interner::Symbol;
use zo_sir::{Insn, LoadSource};

use rustc_hash::FxHashMap as HashMap;

/// Callee-saved general-purpose registers we may claim for
/// promotion, in assignment order. x19..x28 are preserved
/// across calls by AAPCS and live outside the register
/// allocator's pool (`ALLOCATABLE_GP`), so claiming them
/// never collides with an allocated temporary.
pub const PROMOTION_REGS: [Register; 10] = [
  Register::new(19),
  Register::new(20),
  Register::new(21),
  Register::new(22),
  Register::new(23),
  Register::new(24),
  Register::new(25),
  Register::new(26),
  Register::new(27),
  Register::new(28),
];

/// Extra weight an access inside a loop body contributes to
/// a local's promotion score, on top of the base count of 1
/// per static access. Nested loops multiply, so a body two
/// loops deep scores `1 + 2 * LOOP_WEIGHT`. The exact value
/// only orders candidates; loops dominate straight-line code
/// by a wide margin, which is the intent.
const LOOP_WEIGHT: u32 = 16;

/// Per-function promotion plan: which scalar local symbols
/// live in which callee-saved register, and the set of
/// registers actually claimed (for prologue save / epilogue
/// restore).
#[derive(Default)]
pub struct Promotion {
  /// `Symbol -> physical register` for every promoted local.
  map: HashMap<u32, Register>,
  /// Registers claimed by this function, in assignment order
  /// (x19 first). Drives the prologue/epilogue save set.
  used: Vec<Register>,
}

/// One promotable scalar local: its symbol id and the
/// loop-weighted count of static accesses (loads + stores).
struct Candidate {
  symbol: u32,
  score: u32,
}

impl Promotion {
  /// The promotion register for `symbol`, if promoted.
  #[inline]
  pub fn reg_of(&self, symbol: Symbol) -> Option<Register> {
    self.map.get(&symbol.as_u32()).copied()
  }

  /// Number of callee-saved registers this function claimed.
  #[inline]
  pub fn used_count(&self) -> usize {
    self.used.len()
  }

  /// The `i`-th claimed register in assignment order.
  ///
  /// @note — Indexed access lets the prologue/epilogue save
  /// loops avoid holding a `&self` borrow across the `&mut
  /// self` emitter calls in the loop body.
  #[inline]
  pub fn used_reg_at(&self, i: usize) -> Register {
    self.used[i]
  }

  /// Build the promotion plan for one function body.
  ///
  /// `is_scalar` classifies a `Store`'s `ty_id.0` as a
  /// register-sized GP scalar (callers wire this to the type
  /// table). A local is a candidate only when every store to
  /// it carries a scalar type — a symbol seen once as an
  /// aggregate store is disqualified outright so we never
  /// promote a struct/array/string local.
  ///
  /// Loads are also scanned so a local that is read but never
  /// stored a non-scalar still ranks by total access count.
  ///
  /// `regs_safe` must be `false` when the function body
  /// contains an instruction whose lowering uses x19..x28 as
  /// ad-hoc scratch (aggregate pretty-printing, array
  /// literal/push/pop). Promotion claims those same
  /// registers, so a clobbering helper would corrupt a
  /// promoted local; the only correct response is to promote
  /// nothing in such a function. See
  /// `ARM64Gen::body_clobbers_promotion_regs`.
  pub fn analyze(
    body: &[Insn],
    is_scalar: impl Fn(u32) -> bool,
    regs_safe: bool,
  ) -> Self {
    if !regs_safe {
      return Self::default();
    }

    let loop_depth = compute_loop_depth(body);

    // `score[sym]` accumulates loop-weighted accesses;
    // `disqualified[sym]` marks any local seen with a
    // non-scalar store, which bars promotion regardless of
    // score.
    let mut score: HashMap<u32, u32> = HashMap::default();
    let mut disqualified: HashMap<u32, ()> = HashMap::default();

    for (i, insn) in body.iter().enumerate() {
      let weight = 1 + LOOP_WEIGHT * loop_depth[i];

      match insn {
        Insn::Store { name, ty_id, .. } => {
          let sym = name.as_u32();

          if is_scalar(ty_id.0) {
            *score.entry(sym).or_insert(0) += weight;
          } else {
            disqualified.insert(sym, ());
          }
        }
        Insn::Load {
          src: LoadSource::Local(sym),
          ..
        } => {
          *score.entry(sym.as_u32()).or_insert(0) += weight;
        }
        _ => {}
      }
    }

    // Only symbols that were a scalar store target are
    // promotable — a pure-load symbol with no store in this
    // function is a parameter alias (handled by the param
    // path) or an outer binding, neither of which this pass
    // owns.
    let mut candidates: Vec<Candidate> = body
      .iter()
      .filter_map(|insn| match insn {
        Insn::Store { name, ty_id, .. }
          if is_scalar(ty_id.0)
            && !disqualified.contains_key(&name.as_u32()) =>
        {
          Some(name.as_u32())
        }
        _ => None,
      })
      .collect::<std::collections::BTreeSet<_>>()
      .into_iter()
      .map(|symbol| Candidate {
        symbol,
        score: score.get(&symbol).copied().unwrap_or(0),
      })
      .collect();

    // Highest score first; ties break on symbol id so the
    // assignment is deterministic across builds.
    candidates
      .sort_by(|a, b| b.score.cmp(&a.score).then(a.symbol.cmp(&b.symbol)));

    let mut map = HashMap::default();
    let mut used = Vec::new();

    for (candidate, &reg) in candidates.iter().zip(PROMOTION_REGS.iter()) {
      map.insert(candidate.symbol, reg);
      used.push(reg);
    }

    Self { map, used }
  }
}

/// Loop-nesting depth at each instruction index.
///
/// A `Jump { target }` whose target label sits at an
/// *earlier* index is a back-edge; every instruction from
/// that label through the jump is one loop level deeper.
/// Overlapping back-edges (nested loops) add up, so the
/// returned depth is the static nesting count — exactly the
/// signal that makes inner-loop locals outrank outer ones.
fn compute_loop_depth(body: &[Insn]) -> Vec<u32> {
  // Label id -> instruction index within this body.
  let mut label_idx: HashMap<u32, usize> = HashMap::default();

  for (i, insn) in body.iter().enumerate() {
    if let Insn::Label { id } = insn {
      label_idx.insert(*id, i);
    }
  }

  // Difference array: +1 at a back-edge's loop entry, -1
  // just past its jump. A prefix sum then yields the depth
  // at every index without nested scans.
  let mut delta = vec![0i32; body.len() + 1];

  for (i, insn) in body.iter().enumerate() {
    if let Insn::Jump { target } = insn
      && let Some(&start) = label_idx.get(target)
      && start <= i
    {
      delta[start] += 1;
      delta[i + 1] -= 1;
    }
  }

  let mut depth = vec![0u32; body.len()];
  let mut running = 0i32;

  for i in 0..body.len() {
    running += delta[i];
    depth[i] = running.max(0) as u32;
  }

  depth
}

#[cfg(test)]
mod tests {
  use super::*;

  use zo_ty::TyId;
  use zo_value::ValueId;

  /// TyId 1 is the only scalar in these tests; any other id
  /// classifies as an aggregate. Keeps the classifier
  /// trivial so the tests exercise the ranking, not the
  /// type table.
  const SCALAR_TY: u32 = 1;
  const AGGREGATE_TY: u32 = 99;

  fn scalar(ty: u32) -> bool {
    ty == SCALAR_TY
  }

  fn store(name: u32, ty: u32) -> Insn {
    Insn::Store {
      name: Symbol::new(name),
      value: ValueId(0),
      ty_id: TyId(ty),
    }
  }

  fn load_local(name: u32) -> Insn {
    Insn::Load {
      dst: ValueId(0),
      src: LoadSource::Local(Symbol::new(name)),
      ty_id: TyId(SCALAR_TY),
    }
  }

  #[test]
  fn promotes_scalar_local() {
    let body = vec![store(10, SCALAR_TY), load_local(10)];
    let plan = Promotion::analyze(&body, scalar, true);

    assert_eq!(plan.reg_of(Symbol::new(10)), Some(PROMOTION_REGS[0]));
    assert_eq!(plan.used_count(), 1);
  }

  #[test]
  fn skips_aggregate_local() {
    // A struct/array local stored once must never promote —
    // its bytes live in memory, not a register.
    let body = vec![store(10, AGGREGATE_TY), load_local(10)];
    let plan = Promotion::analyze(&body, scalar, true);

    assert_eq!(plan.reg_of(Symbol::new(10)), None);
    assert!(plan.used_count() == 0);
  }

  #[test]
  fn disqualifies_local_with_any_aggregate_store() {
    // A symbol seen once as an aggregate store is barred even
    // if other stores look scalar — never promote a value
    // whose backing shape is sometimes memory.
    let body = vec![store(10, SCALAR_TY), store(10, AGGREGATE_TY)];
    let plan = Promotion::analyze(&body, scalar, true);

    assert_eq!(plan.reg_of(Symbol::new(10)), None);
  }

  #[test]
  fn unsafe_regs_promote_nothing() {
    // When the function clobbers x19..x28 (aggregate print,
    // array op), promotion must be disabled entirely — a
    // clobbering helper would corrupt any promoted local.
    let body = vec![store(10, SCALAR_TY), load_local(10)];
    let plan = Promotion::analyze(&body, scalar, false);

    assert_eq!(plan.reg_of(Symbol::new(10)), None);
    assert_eq!(plan.used_count(), 0);
  }

  #[test]
  fn loop_accesses_outrank_straightline() {
    // `hot` (sym 20) is accessed once per loop iteration;
    // `cold` (sym 21) only in straight-line code. Even with
    // 11 cold accesses vs 1 hot loop access, the loop weight
    // (16) makes `hot` win the single available register.
    let mut body = vec![store(20, SCALAR_TY), store(21, SCALAR_TY)];

    // 11 straight-line loads of cold.
    for _ in 0..11 {
      body.push(load_local(21));
    }

    // One loop containing a single load of hot:
    // L0: load hot ; jmp L0
    body.push(Insn::Label { id: 0 });
    body.push(load_local(20));
    body.push(Insn::Jump { target: 0 });

    let plan = Promotion::analyze(&body, scalar, true);

    // hot's loop access: weight 1 + 16 = 17, plus its store
    // (weight 1) = 18. cold: 1 store + 11 loads = 12. hot
    // wins, and with only the first register both still fit,
    // so assert the ORDER: hot gets x19.
    assert_eq!(plan.reg_of(Symbol::new(20)), Some(PROMOTION_REGS[0]));
    assert_eq!(plan.reg_of(Symbol::new(21)), Some(PROMOTION_REGS[1]));
  }

  #[test]
  fn caps_at_available_registers() {
    // More scalar locals than callee-saved registers: only
    // the top `PROMOTION_REGS.len()` promote; the rest stay
    // in memory (mapped to None).
    let count = PROMOTION_REGS.len() as u32 + 4;
    let body: Vec<Insn> =
      (0..count).map(|s| store(100 + s, SCALAR_TY)).collect();

    let plan = Promotion::analyze(&body, scalar, true);

    assert_eq!(plan.used_count(), PROMOTION_REGS.len());

    let promoted = (0..count)
      .filter(|s| plan.reg_of(Symbol::new(100 + s)).is_some())
      .count();

    assert_eq!(promoted, PROMOTION_REGS.len());
  }

  #[test]
  fn nested_loops_weight_deeper() {
    let depth = compute_loop_depth(&[
      Insn::Label { id: 0 },    // 0  outer entry
      Insn::Label { id: 1 },    // 1  inner entry
      load_local(1),            // 2  depth 2
      Insn::Jump { target: 1 }, // 3  inner back-edge
      load_local(2),            // 4  depth 1
      Insn::Jump { target: 0 }, // 5  outer back-edge
    ]);

    assert_eq!(depth[2], 2);
    assert_eq!(depth[4], 1);
  }

  #[test]
  fn forward_jump_is_not_a_loop() {
    // A jump to a LATER label (forward branch, e.g. an early
    // return) must not register as a loop.
    let depth = compute_loop_depth(&[
      Insn::Jump { target: 0 }, // 0  forward jump
      load_local(1),            // 1
      Insn::Label { id: 0 },    // 2  target after the jump
    ]);

    assert_eq!(depth, vec![0, 0, 0]);
  }
}
