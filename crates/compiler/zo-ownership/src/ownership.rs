//! Affine ownership / move checking — PLAN_MEMORY_MANAGEMENT A2.
//!
//! Forward bitvector dataflow over the SIR. One fact per affine
//! binding — "the value was moved" — set when the binding is the
//! receiver of a consuming (`own self`) call, cleared on
//! reassignment, joined by union (moved on any path ⇒ moved). A
//! read of a moved binding is `UseAfterMove`. This catches
//! double-free and use-after-free on the manual `.free()` /
//! `.close()` destructors.

use zo_error::{Error, ErrorKind};
use zo_interner::Symbol;
use zo_liveness::{BitVec, Cfg, insn_def};
use zo_reporter::report_error;
use zo_sir::{Insn, LoadSource, Sir};
use zo_span::Span;
use zo_ty::SelfKind;
use zo_value::ValueId;

use rustc_hash::FxHashMap as HashMap;

/// The move-checking pass over a whole SIR program.
pub struct Ownership<'sir> {
  sir: &'sir Sir,
}

impl<'sir> Ownership<'sir> {
  /// Binds the pass to a SIR program.
  pub fn new(sir: &'sir Sir) -> Self {
    Self { sir }
  }

  /// Checks every function body and reports `UseAfterMove`.
  pub fn check(&self) {
    let kinds = self.self_kinds();
    let insns = &self.sir.instructions;
    let n = insns.len();
    let mut i = 0;

    while i < n {
      let Insn::FunDef { span, .. } = &insns[i] else {
        i += 1;
        continue;
      };

      // Body range `[start, end)` — `end` is the next `FunDef`.
      let start = i;
      let mut end = i + 1;
      while end < n && !matches!(insns[end], Insn::FunDef { .. }) {
        end += 1;
      }

      self.check_fn(start, end, *span, &kinds);
      i = end;
    }
  }

  /// Mangled function name → receiver mode, for matching a
  /// `Call` against a consuming (`own self`) callee.
  fn self_kinds(&self) -> HashMap<Symbol, SelfKind> {
    let mut kinds = HashMap::default();

    for insn in &self.sir.instructions {
      if let Insn::FunDef {
        name, self_kind, ..
      } = insn
      {
        kinds.insert(*name, *self_kind);
      }
    }

    kinds
  }

  /// Move-checks one function body `[start, end)`.
  fn check_fn(
    &self,
    start: usize,
    end: usize,
    fn_span: Span,
    kinds: &HashMap<Symbol, SelfKind>,
  ) {
    let insns = &self.sir.instructions;

    // Def-site index for this body: value → defining insn. A
    // per-function map, not a value-sized dense array — merged
    // SIR reuses `ValueId`s across functions, so a global dense
    // table both misroutes and over-allocates.
    let mut def_site: HashMap<ValueId, usize> = HashMap::default();
    for (offset, insn) in insns[(start + 1)..end].iter().enumerate() {
      if let Some(dst) = insn_def(insn) {
        def_site.insert(dst, start + 1 + offset);
      }
    }

    // Consuming call sites: insn index → moved binding. Each
    // distinct affine binding also gets a dense bit index.
    let mut bit_of: HashMap<Symbol, usize> = HashMap::default();
    let mut moved_by: HashMap<usize, Symbol> = HashMap::default();

    for (offset, insn) in insns[(start + 1)..end].iter().enumerate() {
      let Insn::Call { name, args, .. } = insn else {
        continue;
      };

      if kinds.get(name) != Some(&SelfKind::Consume) {
        continue;
      }

      // The receiver is the first argument; trace it to the
      // `Load Local(sym)` that produced it.
      let Some(recv) = args.first() else { continue };
      let Some(&def_idx) = def_site.get(recv) else {
        continue;
      };

      if let Insn::Load {
        src: LoadSource::Local(sym),
        ..
      } = &insns[def_idx]
      {
        let next = bit_of.len();

        bit_of.entry(*sym).or_insert(next);
        moved_by.insert(start + 1 + offset, *sym);
      }
    }

    // No binding is ever moved here — nothing to track.
    if bit_of.is_empty() {
      return;
    }

    let nbits = bit_of.len();
    let cfg = Cfg::build(insns, start, end);
    let nblocks = cfg.blocks.len();

    // Forward fixed-point: `in[b] = ∪ out[pred]`, `out =
    // transfer(in)`. The union join makes a value moved on any
    // path moved at the merge.
    let mut in_set = vec![BitVec::new(nbits); nblocks];
    let mut out_set = vec![BitVec::new(nbits); nblocks];

    let mut changed = true;
    while changed {
      changed = false;

      for b in 0..nblocks {
        let mut new_in = BitVec::new(nbits);
        for &p in &cfg.blocks[b].preds {
          new_in.union_with(&out_set[p]);
        }

        let mut cur = new_in.clone();
        let (b_start, b_end) = (cfg.blocks[b].start, cfg.blocks[b].end);
        for (offset, insn) in insns[b_start..b_end].iter().enumerate() {
          self.transfer(insn, b_start + offset, &bit_of, &moved_by, &mut cur);
        }

        if new_in != in_set[b] || cur != out_set[b] {
          in_set[b] = new_in;
          out_set[b] = cur;
          changed = true;
        }
      }
    }

    // Reporting pass: replay each block from its in-state and
    // flag a read of a moved binding. Runs once per block, so an
    // offending use is reported exactly once.
    for (cur_in, block) in in_set.iter().zip(&cfg.blocks) {
      let mut cur = cur_in.clone();

      for (offset, insn) in insns[block.start..block.end].iter().enumerate() {
        if let Insn::Load {
          src: LoadSource::Local(sym),
          ..
        } = insn
          && let Some(&bit) = bit_of.get(sym)
          && cur.test(bit)
        {
          report_error(Error::new(ErrorKind::UseAfterMove, fn_span));
        }

        self.transfer(insn, block.start + offset, &bit_of, &moved_by, &mut cur);
      }
    }
  }

  /// One instruction's effect on the moved-set: a store to a
  /// tracked binding clears it (reassignment resurrects the
  /// value); a consuming call sets it (the receiver is moved).
  fn transfer(
    &self,
    insn: &Insn,
    idx: usize,
    bit_of: &HashMap<Symbol, usize>,
    moved_by: &HashMap<usize, Symbol>,
    cur: &mut BitVec,
  ) {
    if let Insn::Store { name, .. } = insn
      && let Some(&bit) = bit_of.get(name)
    {
      cur.unset(bit);
    }

    if let Some(sym) = moved_by.get(&idx)
      && let Some(&bit) = bit_of.get(sym)
    {
      cur.set(bit);
    }
  }
}
