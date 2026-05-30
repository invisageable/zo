//! Affine ownership / move checking + drop elaboration.
//!
//! A single forward pass over the post-DCE SIR that (1) rejects
//! using a value after it is consumed and (2) elaborates the
//! executor's scope-exit `Insn::Drop` markers into real
//! destructor calls — deterministic RAII, no reference counting,
//! no drop flags.
//!
//! Move facts (one bit per affine binding, "the value was
//! moved") are produced by three triggers:
//! - the receiver of a consuming (`own self`) call,
//! - a whole-value copy `imu w := v` into an owned binding,
//! - `return v` of an owned binding.
//!
//! Reassignment clears the bit. Two joins run on the shared
//! `zo-liveness` scaffold: union (moved on ANY path) drives the
//! use-after-move / double-free diagnostics and conditional-free
//! detection; intersection (moved on ALL paths) drives drop
//! elision.
//!
//! Each `Insn::Drop { local, ty_id }` resolves to one of:
//! - the type has no destructor, or the binding aliases a value
//!   it does not own → ELIDE,
//! - the binding is moved on all paths reaching the drop → ELIDE
//!   (the move already freed it),
//! - moved on some-but-not-all paths → `ConditionalMove` error,
//! - otherwise → KEEP, lowered to `Load Local; Call <dtor>` so
//!   codegen reuses the ordinary manual-free path.

use zo_error::{Error, ErrorKind};
use zo_interner::{Interner, Symbol};
use zo_liveness::{BitVec, Cfg, insn_def};
use zo_reporter::report_error;
use zo_sir::{Insn, LoadSource, Sir};
use zo_span::Span;
use zo_ty::{SelfKind, Ty, TyId};
use zo_ty_checker::TyChecker;
use zo_value::ValueId;

use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};

/// A type's destructor — its unique consuming (`own self`)
/// method — resolved from the merged `FunDef`s.
#[derive(Clone, Copy, Debug)]
struct Dtor {
  /// Mangled `Type::method` name codegen forms its call from.
  name: Symbol,
  /// Pack that owns the destructor body (`callee_pack`).
  pack: Option<Symbol>,
  /// The destructor's return type (the synthesized call's
  /// `ty_id`).
  ret_ty: TyId,
}

/// What a type name resolves to in the destructor table. A type
/// exposing more than one distinct `own self` method is
/// `Ambiguous` — v1 declines to guess which consume frees, so it
/// gets no auto-drop (same outcome as no destructor at all).
#[derive(Clone, Copy, Debug)]
enum DtorSlot {
  Unique(Dtor),
  Ambiguous,
}

/// Per-function move facts, gathered in one scan and then
/// consumed by the dataflow and the reporting/elision replay.
#[derive(Default)]
struct Facts {
  /// Each moved binding's dense bit index.
  bit_of: HashMap<Symbol, usize>,
  /// Instruction index → the binding it moves.
  move_set_at: HashMap<usize, Symbol>,
  /// The subset of `move_set_at` that are consuming calls —
  /// drives `DoubleFree` (vs `UseAfterMove`).
  consume_call_at: HashMap<usize, Symbol>,
  /// Receiver `Load`s of consuming calls, reported at the call
  /// rather than twice.
  consume_recv_loads: HashSet<usize>,
  /// First move site per binding (the secondary diagnostic).
  move_span: HashMap<Symbol, Span>,
  /// Bindings that alias a value they do not own — their drops
  /// always elide.
  borrowed: HashSet<Symbol>,
}

impl Facts {
  /// Intern a dense bit for `sym`, allocating on first use.
  fn bit_for(&mut self, sym: Symbol) -> usize {
    let next = self.bit_of.len();

    *self.bit_of.entry(sym).or_insert(next)
  }
}

/// The intrinsic data of one `Insn::Drop` under decision.
#[derive(Clone, Copy, Debug)]
struct DropPoint {
  local: Symbol,
  ty_id: TyId,
  span: Span,
}

/// What to do with one `Insn::Drop` at compaction time.
enum DropAction {
  /// Remove the marker — already freed, moved, or no
  /// destructor exists.
  Elide,
  /// Lower to `Load Local(local); Call dtor(loaded)`.
  Lower {
    local: Symbol,
    ty_id: TyId,
    dtor: Dtor,
  },
}

/// The move-checking + drop-elaboration pass over a whole SIR
/// program.
pub struct Ownership<'a> {
  sir: &'a mut Sir,
  interner: &'a Interner,
  ty: &'a TyChecker,
}

impl<'a> Ownership<'a> {
  /// Binds the pass to a SIR program and its type/name context.
  pub fn new(
    sir: &'a mut Sir,
    interner: &'a Interner,
    ty: &'a TyChecker,
  ) -> Self {
    Self { sir, interner, ty }
  }

  /// Checks every function body, reports move violations, and
  /// elaborates the surviving scope-exit drops.
  pub fn check(&mut self) {
    // The span side-table must stay aligned with instructions
    // before any span lookup. A mismatch is a compiler bug,
    // not a user error — surface it as such rather than read a
    // desynced array.
    if self.sir.instructions.len() != self.sir.spans.len() {
      report_error(Error::new(ErrorKind::InternalCompilerError, Span::ZERO));

      return;
    }

    let (kinds, dtors) = self.self_kinds_and_destructors();

    // Drop actions keyed by ORIGINAL instruction index. The
    // analysis below indexes into `self.sir.instructions`
    // unchanged; the single compaction at the end applies them.
    let mut actions: HashMap<usize, DropAction> = HashMap::default();

    let n = self.sir.instructions.len();
    let mut i = 0;

    while i < n {
      if !matches!(self.sir.instructions[i], Insn::FunDef { .. }) {
        i += 1;
        continue;
      }

      // Body range `[start, end)` — `end` is the next `FunDef`.
      let start = i;
      let mut end = i + 1;
      while end < n
        && !matches!(self.sir.instructions[end], Insn::FunDef { .. })
      {
        end += 1;
      }

      self.check_fn(start, end, &kinds, &dtors, &mut actions);
      i = end;
    }

    self.elaborate(actions);
  }

  /// One pass over the instruction stream yielding both:
  /// - `kinds`: mangled function name → receiver mode, for
  ///   matching a `Call` against a consuming (`own self`)
  ///   callee;
  /// - `dtors`: type name → its destructor (its unique `own
  ///   self` method — `Vec::free`, a user struct's consuming
  ///   method, …). A type exposing more than one distinct
  ///   consuming method is ambiguous and gets no auto-drop
  ///   (`DtorSlot::Ambiguous`); v1 stays conservative rather than
  ///   guess which consume frees. Keyed by the type-name string so no
  ///   interning (`&mut Interner`) is needed here.
  fn self_kinds_and_destructors(
    &self,
  ) -> (HashMap<Symbol, SelfKind>, HashMap<String, DtorSlot>) {
    let mut kinds = HashMap::default();
    let mut dtors: HashMap<String, DtorSlot> = HashMap::default();

    for insn in &self.sir.instructions {
      let Insn::FunDef {
        name,
        self_kind,
        owning_pack,
        return_ty,
        ..
      } = insn
      else {
        continue;
      };

      kinds.insert(*name, *self_kind);

      if *self_kind != SelfKind::Consume {
        continue;
      }

      let full = self.interner.get(*name);
      let Some((ty_str, _method)) = full.rsplit_once("::") else {
        continue;
      };

      let dtor = Dtor {
        name: *name,
        pack: *owning_pack,
        ret_ty: *return_ty,
      };

      dtors
        .entry(ty_str.to_string())
        .and_modify(|slot| {
          // A second, distinct consuming method on the same
          // type is ambiguous — disable auto-drop for it.
          if !matches!(slot, DtorSlot::Unique(d) if d.name == dtor.name) {
            *slot = DtorSlot::Ambiguous;
          }
        })
        .or_insert(DtorSlot::Unique(dtor));
    }

    (kinds, dtors)
  }

  /// Resolve the destructor for a dropped value's type, or
  /// `None` when the type has no name (array / tuple / …) or
  /// no unique destructor.
  fn dtor_of(
    &self,
    ty_id: TyId,
    dtors: &HashMap<String, DtorSlot>,
  ) -> Option<Dtor> {
    let name = match self.ty.resolve_ty(ty_id) {
      Ty::Struct(sid) => self.ty.ty_table.struct_ty(sid).map(|s| s.name),
      Ty::Enum(eid) => self.ty.ty_table.enum_ty(eid).map(|e| e.name),
      _ => None,
    }?;

    match dtors.get(self.interner.get(name)) {
      Some(DtorSlot::Unique(dtor)) => Some(*dtor),
      // Absent or ambiguous → no auto-drop.
      _ => None,
    }
  }

  /// Move-checks one function body `[start, end)`, reporting
  /// violations and recording a `DropAction` for every
  /// `Insn::Drop` it contains.
  fn check_fn(
    &self,
    start: usize,
    end: usize,
    kinds: &HashMap<Symbol, SelfKind>,
    dtors: &HashMap<String, DtorSlot>,
    actions: &mut HashMap<usize, DropAction>,
  ) {
    let insns = &self.sir.instructions;
    let spans = &self.sir.spans;
    let body = (start + 1)..end;

    // Def-site index for this body: value → defining insn. A
    // per-function map, not a value-sized dense array — merged
    // SIR reuses `ValueId`s across functions.
    let mut def_site: HashMap<ValueId, usize> = HashMap::default();
    for (offset, insn) in insns[body.clone()].iter().enumerate() {
      if let Some(dst) = insn_def(insn) {
        def_site.insert(dst, start + 1 + offset);
      }
    }

    // Owned bindings = drop-marked locals whose type actually
    // has a destructor. The executor marks every `Struct`/`Enum`
    // local for drop (it cannot resolve destructors before the
    // stdlib merges), so the marker set over-approximates: a
    // plain value struct (`Rect`) gets a marker but is Copy, and
    // `imu r2 := r1` is a copy, not a move. Gating the affine
    // set on a real destructor keeps those copies off the
    // move-tracker; their destructor-less drops elide anyway.
    let mut owned: HashSet<Symbol> = HashSet::default();
    let mut has_drop = false;
    for insn in &insns[body.clone()] {
      if let Insn::Drop { local, ty_id } = insn {
        has_drop = true;

        if self.dtor_of(*ty_id, dtors).is_some() {
          owned.insert(*local);
        }
      }
    }

    // Move facts gathered in one scan.
    let mut facts = Facts::default();

    for (offset, insn) in insns[body.clone()].iter().enumerate() {
      let idx = start + 1 + offset;

      match insn {
        // Consuming (`own self`) call: the receiver is moved.
        Insn::Call { name, args, .. }
          if kinds.get(name) == Some(&SelfKind::Consume) =>
        {
          let Some(recv) = args.first() else { continue };
          let Some(&def_idx) = def_site.get(recv) else {
            continue;
          };

          if let Insn::Load {
            src: LoadSource::Local(sym),
            ..
          } = &insns[def_idx]
          {
            facts.bit_for(*sym);
            facts.move_set_at.insert(idx, *sym);
            facts.consume_call_at.insert(idx, *sym);
            facts.move_span.entry(*sym).or_insert(spans[idx]);
            facts.consume_recv_loads.insert(def_idx);
          }
        }

        // Whole-value copy `… := v`. If `v` is owned this moves
        // it (the source drop elides); if `v` is a borrow /
        // parameter the destination aliases a value it does not
        // own, so the destination's drop elides instead.
        Insn::Store {
          name: dst, value, ..
        } => {
          if let Some(&def_idx) = def_site.get(value)
            && let Insn::Load {
              src: LoadSource::Local(src),
              ..
            } = &insns[def_idx]
            && src != dst
          {
            if owned.contains(src) {
              facts.bit_for(*src);
              facts.move_set_at.insert(def_idx, *src);
              facts.move_span.entry(*src).or_insert(spans[def_idx]);
            } else {
              facts.borrowed.insert(*dst);
            }
          }
        }

        // `return v` moves the returned owned binding out.
        Insn::Return { value: Some(v), .. } => {
          if let Some(&def_idx) = def_site.get(v)
            && let Insn::Load {
              src: LoadSource::Local(src),
              ..
            } = &insns[def_idx]
            && owned.contains(src)
          {
            facts.bit_for(*src);
            facts.move_set_at.insert(def_idx, *src);
            facts.move_span.entry(*src).or_insert(spans[def_idx]);
          }
        }

        _ => {}
      }
    }

    // Nothing affine and no drops to elaborate.
    if facts.bit_of.is_empty() && !has_drop {
      return;
    }

    // Drops but no moves — the common "owns a value, never moves
    // it" function. With no move bits the dataflow is a no-op:
    // every drop is decided from its type and borrow status
    // alone, so skip the CFG build and both fixed-point solves.
    if facts.bit_of.is_empty() {
      let empty = BitVec::new(0);
      let reported = HashSet::default();

      for (offset, insn) in insns[body.clone()].iter().enumerate() {
        if let Insn::Drop { local, ty_id } = insn {
          let idx = start + 1 + offset;
          let point = DropPoint {
            local: *local,
            ty_id: *ty_id,
            span: spans[idx],
          };

          let action =
            self.decide_drop(point, &facts, dtors, &reported, &empty, &empty);

          actions.insert(idx, action);
        }
      }

      return;
    }

    let nbits = facts.bit_of.len();
    let cfg = Cfg::build(insns, start, end);

    let in_union = solve(
      insns,
      &cfg,
      nbits,
      &facts.bit_of,
      &facts.move_set_at,
      Join::Union,
    );
    let in_inter = solve(
      insns,
      &cfg,
      nbits,
      &facts.bit_of,
      &facts.move_set_at,
      Join::Intersect,
    );

    // Single replay per block: report violations from the union
    // state, decide each drop from both states. Runs once per
    // block, so an offending use is reported once. `reported`
    // accumulates across blocks in program order — a binding
    // flagged at an earlier use suppresses the conditional-free
    // diagnostic at its later scope-exit drop.
    let mut reported: HashSet<Symbol> = HashSet::default();

    for (b, block) in cfg.blocks.iter().enumerate() {
      let mut union_cur = in_union[b].clone();
      let mut inter_cur = in_inter[b].clone();

      for (offset, insn) in insns[block.start..block.end].iter().enumerate() {
        let idx = block.start + offset;

        if let Some(sym) = self.report_use(insn, idx, &union_cur, &facts, spans)
        {
          reported.insert(sym);
        }

        if let Insn::Drop { local, ty_id } = insn {
          let point = DropPoint {
            local: *local,
            ty_id: *ty_id,
            span: spans[idx],
          };

          let action = self.decide_drop(
            point, &facts, dtors, &reported, &union_cur, &inter_cur,
          );

          actions.insert(idx, action);
        }

        transfer(insn, idx, &facts.bit_of, &facts.move_set_at, &mut union_cur);
        transfer(insn, idx, &facts.bit_of, &facts.move_set_at, &mut inter_cur);
      }
    }
  }

  /// Report a use of an already-moved binding: consuming it
  /// again is a `DoubleFree`, reading it is a `UseAfterMove`.
  /// Returns the offending binding so its scope-exit drop can
  /// be silently elided — the use is the real error, no need to
  /// also flag the drop as a conditional free.
  fn report_use(
    &self,
    insn: &Insn,
    idx: usize,
    union_cur: &BitVec,
    facts: &Facts,
    spans: &[Span],
  ) -> Option<Symbol> {
    if let Some(sym) = facts.consume_call_at.get(&idx)
      && let Some(&bit) = facts.bit_of.get(sym)
      && union_cur.test(bit)
    {
      let moved_at = facts.move_span.get(sym).copied().unwrap_or(spans[idx]);

      report_error(Error::with_secondary(
        ErrorKind::DoubleFree,
        spans[idx],
        moved_at,
      ));

      return Some(*sym);
    } else if let Insn::Load {
      src: LoadSource::Local(sym),
      ..
    } = insn
      && !facts.consume_recv_loads.contains(&idx)
      && let Some(&bit) = facts.bit_of.get(sym)
      && union_cur.test(bit)
    {
      let moved_at = facts.move_span.get(sym).copied().unwrap_or(spans[idx]);

      report_error(Error::with_secondary(
        ErrorKind::UseAfterMove,
        spans[idx],
        moved_at,
      ));

      return Some(*sym);
    }

    None
  }

  /// Decide a single drop. Order matters: a borrow or a
  /// destructor-less type elides unconditionally; otherwise the
  /// move state on the paths reaching the drop decides.
  fn decide_drop(
    &self,
    point: DropPoint,
    facts: &Facts,
    dtors: &HashMap<String, DtorSlot>,
    reported: &HashSet<Symbol>,
    union_cur: &BitVec,
    inter_cur: &BitVec,
  ) -> DropAction {
    let DropPoint { local, ty_id, span } = point;

    // A binding that aliases a value it does not own, or one
    // already flagged as used-after-move / double-freed, has
    // no live drop to lower — and re-flagging it as a
    // conditional free would just be noise.
    if facts.borrowed.contains(&local) || reported.contains(&local) {
      return DropAction::Elide;
    }

    let Some(dtor) = self.dtor_of(ty_id, dtors) else {
      return DropAction::Elide;
    };

    let bit = facts.bit_of.get(&local).copied();
    let moved_all = bit.is_some_and(|b| inter_cur.test(b));
    let moved_some = bit.is_some_and(|b| union_cur.test(b));

    if moved_all {
      DropAction::Elide
    } else if moved_some {
      // Freed on some paths but not all — a single static drop
      // would double-free or leak depending on the path.
      report_error(Error::new(ErrorKind::ConditionalMove, span));

      DropAction::Elide
    } else {
      DropAction::Lower { local, ty_id, dtor }
    }
  }

  /// Apply the drop actions: elide removes the marker, keep
  /// lowers it to `Load Local; Call <dtor>`. Rebuilds
  /// `instructions` / `spans` in one pass so they stay aligned,
  /// minting fresh `ValueId`s for the synthesized insns.
  fn elaborate(&mut self, actions: HashMap<usize, DropAction>) {
    if actions.is_empty() {
      return;
    }

    let old = std::mem::take(&mut self.sir.instructions);
    let old_spans = std::mem::take(&mut self.sir.spans);
    let mut next_value = self.sir.next_value_id;

    let mut insns = Vec::with_capacity(old.len());
    let mut spans = Vec::with_capacity(old.len());

    for (i, insn) in old.into_iter().enumerate() {
      let span = old_spans[i];

      match actions.get(&i) {
        Some(DropAction::Elide) => {}
        Some(DropAction::Lower { local, ty_id, dtor }) => {
          let loaded = ValueId(next_value);
          next_value += 1;

          insns.push(Insn::Load {
            dst: loaded,
            src: LoadSource::Local(*local),
            ty_id: *ty_id,
          });
          spans.push(span);

          let result = ValueId(next_value);
          next_value += 1;

          insns.push(Insn::Call {
            dst: result,
            name: dtor.name,
            callee_pack: dtor.pack,
            args: vec![loaded],
            ty_id: dtor.ret_ty,
          });
          spans.push(span);
        }
        None => {
          // A stray `Drop` outside any analyzed function body
          // (none in practice) elides rather than reach codegen.
          if matches!(insn, Insn::Drop { .. }) {
            continue;
          }

          insns.push(insn);
          spans.push(span);
        }
      }
    }

    self.sir.instructions = insns;
    self.sir.spans = spans;
    self.sir.next_value_id = next_value;
  }
}

/// The merge operator for a forward solve.
#[derive(Clone, Copy)]
enum Join {
  /// Moved on ANY predecessor (use-after-move, double-free).
  Union,
  /// Moved on EVERY predecessor (drop elision, must-analysis).
  Intersect,
}

/// One instruction's effect on the moved-set: a store to a
/// tracked binding clears it (reassignment resurrects the
/// value); a recorded move site sets it.
fn transfer(
  insn: &Insn,
  idx: usize,
  bit_of: &HashMap<Symbol, usize>,
  move_set_at: &HashMap<usize, Symbol>,
  cur: &mut BitVec,
) {
  if let Insn::Store { name, .. } = insn
    && let Some(&bit) = bit_of.get(name)
  {
    cur.unset(bit);
  }

  if let Some(sym) = move_set_at.get(&idx)
    && let Some(&bit) = bit_of.get(sym)
  {
    cur.set(bit);
  }
}

/// Forward fixed-point over the function CFG. Union seeds every
/// block at ∅ and grows; intersection seeds non-entry blocks at
/// the universe (top) and shrinks. A block with no predecessors
/// (entry, or unreachable) takes ∅ either way. Returns the
/// in-state per block.
fn solve(
  insns: &[Insn],
  cfg: &Cfg,
  nbits: usize,
  bit_of: &HashMap<Symbol, usize>,
  move_set_at: &HashMap<usize, Symbol>,
  join: Join,
) -> Vec<BitVec> {
  let nblocks = cfg.blocks.len();

  let mut in_set = vec![BitVec::new(nbits); nblocks];
  let mut out_set = match join {
    Join::Union => vec![BitVec::new(nbits); nblocks],
    Join::Intersect => vec![BitVec::new_full(nbits); nblocks],
  };

  let mut changed = true;
  while changed {
    changed = false;

    for b in 0..nblocks {
      let preds = &cfg.blocks[b].preds;

      let new_in = if preds.is_empty() {
        BitVec::new(nbits)
      } else {
        match join {
          Join::Union => {
            let mut acc = BitVec::new(nbits);
            for &p in preds {
              acc.union_with(&out_set[p]);
            }
            acc
          }
          Join::Intersect => {
            let mut acc = out_set[preds[0]].clone();
            for &p in &preds[1..] {
              acc.intersect_with(&out_set[p]);
            }
            acc
          }
        }
      };

      let mut cur = new_in.clone();
      let (b_start, b_end) = (cfg.blocks[b].start, cfg.blocks[b].end);
      for (offset, insn) in insns[b_start..b_end].iter().enumerate() {
        transfer(insn, b_start + offset, bit_of, move_set_at, &mut cur);
      }

      if new_in != in_set[b] || cur != out_set[b] {
        in_set[b] = new_in;
        out_set[b] = cur;
        changed = true;
      }
    }
  }

  in_set
}
