//! Affine ownership / move checking.
//!
//! Forward bitvector dataflow over the SIR. One fact per affine
//! binding — "the value was moved" — set when the binding is the
//! receiver of a consuming (`own self`) call, cleared on
//! reassignment, joined by union (moved on any path ⇒ moved).
//!
//! Two violations, distinguished by what the offending use is:
//! - consuming an already-moved binding → `DoubleFree`,
//! - reading an already-moved binding → `UseAfterMove`.
//!
//! Both report two spans: the offending use (primary) and the
//! consume that moved it (secondary), read from `Sir::spans`.

use zo_error::{Error, ErrorKind};
use zo_interner::Symbol;
use zo_liveness::{BitVec, Cfg, insn_def};
use zo_reporter::report_error;
use zo_sir::{Insn, LoadSource, Sir};
use zo_span::Span;
use zo_ty::SelfKind;
use zo_value::ValueId;

use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};

/// The move-checking pass over a whole SIR program.
pub struct Ownership<'sir> {
  sir: &'sir Sir,
}

impl<'sir> Ownership<'sir> {
  /// Binds the pass to a SIR program.
  pub fn new(sir: &'sir Sir) -> Self {
    Self { sir }
  }

  /// Checks every function body and reports move violations.
  pub fn check(&self) {
    // The span side-table must stay aligned with instructions
    // before any span lookup. A mismatch is a compiler bug,
    // not a user error — surface it as such rather than read a
    // desynced array.
    if self.sir.instructions.len() != self.sir.spans.len() {
      report_error(Error::new(ErrorKind::InternalCompilerError, Span::ZERO));

      return;
    }

    let kinds = self.self_kinds();
    let insns = &self.sir.instructions;
    let n = insns.len();
    let mut i = 0;

    while i < n {
      if !matches!(insns[i], Insn::FunDef { .. }) {
        i += 1;
        continue;
      }

      // Body range `[start, end)` — `end` is the next `FunDef`.
      let start = i;
      let mut end = i + 1;
      while end < n && !matches!(insns[end], Insn::FunDef { .. }) {
        end += 1;
      }

      self.check_fn(start, end, &kinds);
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
    kinds: &HashMap<Symbol, SelfKind>,
  ) {
    let insns = &self.sir.instructions;
    let spans = &self.sir.spans;

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

    // Consuming call sites. Each distinct affine binding gets a
    // dense bit index; `moved_by` maps a consuming call to the
    // binding it moves; `move_span` records where a binding was
    // first moved (the secondary diagnostic span);
    // `consume_recv_loads` marks the receiver `Load`s so they
    // are reported as `DoubleFree` at the call, not twice.
    let mut bit_of: HashMap<Symbol, usize> = HashMap::default();
    let mut moved_by: HashMap<usize, Symbol> = HashMap::default();
    let mut move_span: HashMap<Symbol, Span> = HashMap::default();
    let mut consume_recv_loads: HashSet<usize> = HashSet::default();

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
        let call_idx = start + 1 + offset;
        let next = bit_of.len();

        bit_of.entry(*sym).or_insert(next);
        moved_by.insert(call_idx, *sym);
        move_span.entry(*sym).or_insert(spans[call_idx]);
        consume_recv_loads.insert(def_idx);
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

    // Reporting pass: replay each block from its in-state. Runs
    // once per block, so an offending use is reported once.
    for (cur_in, block) in in_set.iter().zip(&cfg.blocks) {
      let mut cur = cur_in.clone();

      for (offset, insn) in insns[block.start..block.end].iter().enumerate() {
        let idx = block.start + offset;

        if let Some(sym) = moved_by.get(&idx)
          && let Some(&bit) = bit_of.get(sym)
          && cur.test(bit)
        {
          // Consuming a binding that was already moved.
          let moved_at = move_span.get(sym).copied().unwrap_or(spans[idx]);

          report_error(Error::with_secondary(
            ErrorKind::DoubleFree,
            spans[idx],
            moved_at,
          ));
        } else if let Insn::Load {
          src: LoadSource::Local(sym),
          ..
        } = insn
          && !consume_recv_loads.contains(&idx)
          && let Some(&bit) = bit_of.get(sym)
          && cur.test(bit)
        {
          // Reading a moved binding (the consume-receiver reads
          // are handled as `DoubleFree` at the call above).
          let moved_at = move_span.get(sym).copied().unwrap_or(spans[idx]);

          report_error(Error::with_secondary(
            ErrorKind::UseAfterMove,
            spans[idx],
            moved_at,
          ));
        }

        self.transfer(insn, idx, &bit_of, &moved_by, &mut cur);
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
