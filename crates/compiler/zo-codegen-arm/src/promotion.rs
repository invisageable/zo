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
//! Scope: general-purpose scalars (`int`, `s8..s64`,
//! `u8..u64`, `bool`, `char`, pointers) lift into x19..x28;
//! `float` locals lift into the callee-saved FP bank d8..d15.
//! The two banks and their register numbers are disjoint, so a
//! function can promote both at once.

use zo_emitter_arm::{FpRegister, Register};
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

/// Callee-saved FP registers we may claim for a promoted
/// `float` local, in assignment order. d8..d15 sit outside the
/// allocator's FP pool (`ALLOCATABLE_FP`), are AAPCS-preserved
/// across calls, and are never used as ad-hoc scratch — so the
/// FP plan needs no clobber gate.
pub const PROMOTION_FREGS: [FpRegister; 8] = [
  FpRegister::new(8),
  FpRegister::new(9),
  FpRegister::new(10),
  FpRegister::new(11),
  FpRegister::new(12),
  FpRegister::new(13),
  FpRegister::new(14),
  FpRegister::new(15),
];

/// Which register bank a local belongs to, by the type of its
/// stores. A local stored under more than one bank — or under a
/// non-register-sized type — is `Other` and promotes nowhere.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LocalKind {
  /// A general-purpose scalar: `int`, `s*`/`u*`, `bool`, `char`,
  /// a pointer. Promotes into x19..x28.
  GpScalar,
  /// A `float`. Promotes into d8..d15.
  Float,
  /// An aggregate, or a conflicting/mixed type. Stays in memory.
  Other,
}

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
  /// `Symbol -> physical register` for every promoted GP local.
  map: HashMap<u32, Register>,
  /// GP registers claimed by this function, in assignment order
  /// (x19 first). Drives the prologue/epilogue save set.
  used: Vec<Register>,
  /// `Symbol -> physical FP register` for every promoted `float`
  /// local.
  fmap: HashMap<u32, FpRegister>,
  /// FP registers claimed by this function, in assignment order
  /// (d8 first). Drives the prologue/epilogue FP save set.
  fp_used: Vec<FpRegister>,
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

  /// The promotion FP register for `symbol`, if promoted.
  #[inline]
  pub fn freg_of(&self, symbol: Symbol) -> Option<FpRegister> {
    self.fmap.get(&symbol.as_u32()).copied()
  }

  /// Number of callee-saved FP registers this function claimed.
  #[inline]
  pub fn fp_used_count(&self) -> usize {
    self.fp_used.len()
  }

  /// The `i`-th claimed FP register in assignment order.
  #[inline]
  pub fn fp_used_reg_at(&self, i: usize) -> FpRegister {
    self.fp_used[i]
  }

  /// Build the promotion plan for one function body.
  ///
  /// `classify` maps a `Store`'s `ty_id.0` to its register bank
  /// (callers wire this to the type table). A local is a
  /// candidate only when every store to it agrees on one bank —
  /// a symbol stored once under a conflicting or aggregate type
  /// collapses to `Other` and promotes nowhere.
  ///
  /// Loads are also scored so a local read more than it is
  /// written still ranks by total access count.
  ///
  /// `regs_safe` gates only the GP bank: it must be `false` when
  /// the body's lowering uses x19..x28 as ad-hoc scratch
  /// (aggregate pretty-printing, array literal/push/pop), since
  /// promotion claims those same registers. The FP bank d8..d15
  /// is never scratch, so the float plan is built regardless.
  /// See `ARM64Gen::body_clobbers_promotion_regs`.
  pub fn analyze(
    body: &[Insn],
    classify: impl Fn(u32) -> LocalKind,
    regs_safe: bool,
  ) -> Self {
    let loop_depth = compute_loop_depth(body);

    // `score` accumulates loop-weighted accesses per local;
    // `kind` records which bank a store target belongs to,
    // collapsing to `Other` on a conflicting or non-register
    // type so such a local promotes nowhere.
    let mut score: HashMap<u32, u32> = HashMap::default();
    let mut kind: HashMap<u32, LocalKind> = HashMap::default();

    for (i, insn) in body.iter().enumerate() {
      let weight = 1 + LOOP_WEIGHT * loop_depth[i];

      match insn {
        Insn::Store { name, ty_id, .. } => {
          let sym = name.as_u32();

          *score.entry(sym).or_insert(0) += weight;

          let observed = classify(ty_id.0);
          let merged = match kind.get(&sym) {
            None => observed,
            Some(prev) if *prev == observed && observed != LocalKind::Other => {
              observed
            }
            _ => LocalKind::Other,
          };

          kind.insert(sym, merged);
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

    let mut map = HashMap::default();
    let mut used = Vec::new();

    // GP plan: gated by `regs_safe`, since x19..x28 double as
    // the aggregate-print / array-op scratch bank.
    if regs_safe {
      for (sym, &reg) in ranked(&kind, &score, LocalKind::GpScalar)
        .iter()
        .zip(&PROMOTION_REGS)
      {
        map.insert(*sym, reg);
        used.push(reg);
      }
    }

    let mut fmap = HashMap::default();
    let mut fp_used = Vec::new();

    // FP plan: always built — d8..d15 are never scratch.
    for (sym, &reg) in ranked(&kind, &score, LocalKind::Float)
      .iter()
      .zip(&PROMOTION_FREGS)
    {
      fmap.insert(*sym, reg);
      fp_used.push(reg);
    }

    Self {
      map,
      used,
      fmap,
      fp_used,
    }
  }
}

/// Store-target symbols of `want` kind, highest loop-weighted
/// score first; ties break on symbol id for a deterministic
/// assignment across builds. A pure-load symbol has no kind
/// entry and is never a candidate.
fn ranked(
  kind: &HashMap<u32, LocalKind>,
  score: &HashMap<u32, u32>,
  want: LocalKind,
) -> Vec<u32> {
  let mut candidates = kind
    .iter()
    .filter(|(_, k)| **k == want)
    .map(|(sym, _)| (*sym, score.get(sym).copied().unwrap_or(0)))
    .collect::<Vec<_>>();

  candidates.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
  candidates.into_iter().map(|(sym, _)| sym).collect()
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
  const FLOAT_TY: u32 = 2;
  const AGGREGATE_TY: u32 = 99;

  fn classify(ty: u32) -> LocalKind {
    match ty {
      SCALAR_TY => LocalKind::GpScalar,
      FLOAT_TY => LocalKind::Float,
      _ => LocalKind::Other,
    }
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
    let plan = Promotion::analyze(&body, classify, true);

    assert_eq!(plan.reg_of(Symbol::new(10)), Some(PROMOTION_REGS[0]));
    assert_eq!(plan.used_count(), 1);
  }

  #[test]
  fn skips_aggregate_local() {
    // A struct/array local stored once must never promote —
    // its bytes live in memory, not a register.
    let body = vec![store(10, AGGREGATE_TY), load_local(10)];
    let plan = Promotion::analyze(&body, classify, true);

    assert_eq!(plan.reg_of(Symbol::new(10)), None);
    assert!(plan.used_count() == 0);
  }

  #[test]
  fn disqualifies_local_with_any_aggregate_store() {
    // A symbol seen once as an aggregate store is barred even
    // if other stores look scalar — never promote a value
    // whose backing shape is sometimes memory.
    let body = vec![store(10, SCALAR_TY), store(10, AGGREGATE_TY)];
    let plan = Promotion::analyze(&body, classify, true);

    assert_eq!(plan.reg_of(Symbol::new(10)), None);
  }

  #[test]
  fn unsafe_regs_promote_nothing() {
    // When the function clobbers x19..x28 (aggregate print,
    // array op), promotion must be disabled entirely — a
    // clobbering helper would corrupt any promoted local.
    let body = vec![store(10, SCALAR_TY), load_local(10)];
    let plan = Promotion::analyze(&body, classify, false);

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

    let plan = Promotion::analyze(&body, classify, true);

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

    let plan = Promotion::analyze(&body, classify, true);

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

  #[test]
  fn promotes_float_local() {
    let body = vec![store(10, FLOAT_TY), load_local(10)];
    let plan = Promotion::analyze(&body, classify, true);

    assert_eq!(plan.freg_of(Symbol::new(10)), Some(PROMOTION_FREGS[0]));
    assert_eq!(plan.fp_used_count(), 1);
    // It claims an FP register, not a GP one.
    assert_eq!(plan.reg_of(Symbol::new(10)), None);
    assert_eq!(plan.used_count(), 0);
  }

  #[test]
  fn caps_at_available_fregs() {
    // More float locals than callee-saved FP registers: only the
    // top `PROMOTION_FREGS.len()` promote; the rest stay in memory.
    let count = PROMOTION_FREGS.len() as u32 + 3;
    let body: Vec<Insn> =
      (0..count).map(|s| store(100 + s, FLOAT_TY)).collect();

    let plan = Promotion::analyze(&body, classify, true);

    assert_eq!(plan.fp_used_count(), PROMOTION_FREGS.len());

    let promoted = (0..count)
      .filter(|s| plan.freg_of(Symbol::new(100 + s)).is_some())
      .count();

    assert_eq!(promoted, PROMOTION_FREGS.len());
  }

  #[test]
  fn int_and_float_coexist() {
    // An int local and a float local in the same body claim
    // disjoint banks (x19 and d8), no collision.
    let body = vec![store(10, SCALAR_TY), store(20, FLOAT_TY)];
    let plan = Promotion::analyze(&body, classify, true);

    assert_eq!(plan.reg_of(Symbol::new(10)), Some(PROMOTION_REGS[0]));
    assert_eq!(plan.freg_of(Symbol::new(20)), Some(PROMOTION_FREGS[0]));
  }

  #[test]
  fn float_promotes_when_regs_unsafe() {
    // `regs_safe = false` gates only the GP bank (x19..x28 double
    // as scratch). d8..d15 are never scratch, so a float local
    // still promotes.
    let body = vec![store(10, SCALAR_TY), store(20, FLOAT_TY)];
    let plan = Promotion::analyze(&body, classify, false);

    assert_eq!(plan.reg_of(Symbol::new(10)), None);
    assert_eq!(plan.freg_of(Symbol::new(20)), Some(PROMOTION_FREGS[0]));
  }
}
