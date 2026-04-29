//! Backward bitvector liveness analysis.

use crate::bitvec::BitVec;
use crate::insn::{insn_var_def, insn_var_use, visit_uses};

use zo_interner::Symbol;
use zo_sir::Insn;
use zo_value::ValueId;

use rustc_hash::FxHashMap as HashMap;

/// Liveness analysis result for a function body.
pub struct LivenessInfo {
  /// Per-instruction (local index) live-in sets, keyed by
  /// the per-function compact bit index. Translate
  /// `ValueId` → bit through [`LivenessInfo::vid_map`] (or
  /// the [`LivenessInfo::is_live_out`] / `is_live_in`
  /// helpers).
  pub live_in: Vec<BitVec>,
  /// Per-instruction (local index) live-out sets, same
  /// indexing as `live_in`.
  pub live_out: Vec<BitVec>,
  /// Per-instruction (local index) live-out sets for named
  /// variables (Symbols). Bit index = var_map[symbol].
  pub var_live_out: Vec<BitVec>,
  /// Maps Symbol → bit index in var bitvectors.
  pub var_map: HashMap<Symbol, usize>,
  /// `ValueId.0` → bit index inside the per-function
  /// `live_in` / `live_out` bitvectors. The per-function
  /// keying replaces a prior whole-program-sized layout
  /// where each BitVec was 689-795 bits but only 5-30
  /// were used per function — this single change moved
  /// liveness from ~55-60% of codegen to ~10%. Values
  /// not referenced in this function are absent from the
  /// map; querying them returns "not live".
  pub vid_map: HashMap<u32, u32>,
}

impl LivenessInfo {
  /// Returns true if the named variable is live-out at
  /// local instruction index `i`.
  pub fn is_var_live_out(&self, i: usize, name: Symbol) -> bool {
    if let Some(&bit) = self.var_map.get(&name) {
      self.var_live_out[i].test(bit)
    } else {
      false
    }
  }

  /// Returns true if `ValueId(vid_raw)` is live-out at
  /// local instruction index `i`. Translates the
  /// whole-program `ValueId` to the per-function bit
  /// index; values not referenced in this function are
  /// reported as not live.
  #[inline]
  pub fn is_live_out_raw(&self, i: usize, vid_raw: u32) -> bool {
    self
      .vid_map
      .get(&vid_raw)
      .is_some_and(|&bit| self.live_out[i].test(bit as usize))
  }
}

/// Backward bitvector liveness analysis over a function body.
///
/// Computes two layers of liveness:
///   - **ValueId liveness** — used by dead instruction
///     elimination and register allocation.
///   - **Variable (Symbol) liveness** — used by dead variable
///     (Store) elimination.
///
/// Both share the same CFG and fixed-point iteration.
pub fn analyze(
  insns: &[Insn],
  start: usize,
  end: usize,
  value_ids: &[Option<ValueId>],
  _num_values: u32,
) -> LivenessInfo {
  let n = end - start;

  // --- assign bit indices to named variables ---

  let mut var_map = HashMap::default();
  let mut next_var_bit = 0usize;

  for i in 0..n {
    let gi = start + i;

    if let Some(sym) = insn_var_def(&insns[gi]) {
      var_map.entry(sym).or_insert_with(|| {
        let bit = next_var_bit;
        next_var_bit += 1;
        bit
      });
    }

    if let Some(sym) = insn_var_use(&insns[gi]) {
      var_map.entry(sym).or_insert_with(|| {
        let bit = next_var_bit;
        next_var_bit += 1;
        bit
      });
    }
  }

  let nvars = next_var_bit;

  // --- assign bit indices to per-function ValueIds ---
  //
  // Sizing the BitVecs to whole-program `num_values` was
  // the dominant cost in codegen — every fixed-point
  // round (~600 rounds × 270 calls) walked a 700-bit
  // bitvec where only 5-30 bits were live. We instead
  // collect every `ValueId` actually referenced in this
  // function and pack them into a compact bit space.
  let mut vid_map: HashMap<u32, u32> = HashMap::default();
  let mut next_vid_bit = 0u32;

  for i in 0..n {
    let gi = start + i;

    if let Some(vid) = value_ids[gi] {
      vid_map.entry(vid.0).or_insert_with(|| {
        let bit = next_vid_bit;
        next_vid_bit += 1;
        bit
      });
    }

    visit_uses(&insns[gi], |u| {
      if u.0 != u32::MAX {
        vid_map.entry(u.0).or_insert_with(|| {
          let bit = next_vid_bit;
          next_vid_bit += 1;
          bit
        });
      }
    });
  }

  let nbits = next_vid_bit as usize;

  // --- defs and uses per local instruction ---

  let mut defs = vec![BitVec::new(nbits); n];
  let mut uses = vec![BitVec::new(nbits); n];
  let mut var_defs = vec![BitVec::new(nvars); n];
  let mut var_uses = vec![BitVec::new(nvars); n];

  for i in 0..n {
    let gi = start + i;

    // ValueId defs/uses.
    if let Some(vid) = value_ids[gi]
      && let Some(&bit) = vid_map.get(&vid.0)
    {
      defs[i].set(bit as usize);
    }

    visit_uses(&insns[gi], |u| {
      if u.0 != u32::MAX
        && let Some(&bit) = vid_map.get(&u.0)
      {
        uses[i].set(bit as usize);
      }
    });

    // Variable (Symbol) defs/uses.
    if let Some(sym) = insn_var_def(&insns[gi])
      && let Some(&bit) = var_map.get(&sym)
    {
      var_defs[i].set(bit);
    }

    if let Some(sym) = insn_var_use(&insns[gi])
      && let Some(&bit) = var_map.get(&sym)
    {
      var_uses[i].set(bit);
    }
  }

  // --- label → local index map ---

  let mut label_map = HashMap::default();

  for i in 0..n {
    if let Insn::Label { id } = &insns[start + i] {
      label_map.insert(*id, i);
    }
  }

  // --- successors ---

  let mut succs = Vec::with_capacity(n);

  for i in 0..n {
    let gi = start + i;
    let mut s = Vec::new();

    match &insns[gi] {
      Insn::Jump { target } => {
        if let Some(&idx) = label_map.get(target) {
          s.push(idx);
        }
      }
      Insn::BranchIfNot { target, .. } => {
        if i + 1 < n {
          s.push(i + 1);
        }

        if let Some(&idx) = label_map.get(target) {
          s.push(idx);
        }
      }
      Insn::Return { .. } => {}
      _ => {
        if i + 1 < n {
          s.push(i + 1);
        }
      }
    }

    succs.push(s);
  }

  // --- fixed-point iteration (both layers) ---

  let mut live_in = vec![BitVec::new(nbits); n];
  let mut live_out = vec![BitVec::new(nbits); n];
  let mut var_live_in = vec![BitVec::new(nvars); n];
  let mut var_live_out = vec![BitVec::new(nvars); n];

  // Hoisted scratch buffers — recycled per insn per
  // round via `copy_from` / `clear`. Replaces the prior
  // `BitVec::new(...) + clone()` pattern that allocated
  // 6 fresh BitVecs per insn per fixed-point round.
  let mut new_out = BitVec::new(nbits);
  let mut new_in = BitVec::new(nbits);
  let mut tmp = BitVec::new(nbits);
  let mut var_new_out = BitVec::new(nvars);
  let mut var_new_in = BitVec::new(nvars);
  let mut var_tmp = BitVec::new(nvars);

  let mut changed = true;

  while changed {
    changed = false;

    for i in (0..n).rev() {
      // --- ValueId layer ---

      new_out.clear();

      for &succ in &succs[i] {
        new_out.union_with(&live_in[succ]);
      }

      if new_out != live_out[i] {
        live_out[i].copy_from(&new_out);
        changed = true;
      }

      new_in.copy_from(&uses[i]);
      tmp.copy_from(&live_out[i]);
      tmp.difference_with(&defs[i]);
      new_in.union_with(&tmp);

      if new_in != live_in[i] {
        live_in[i].copy_from(&new_in);
        changed = true;
      }

      // --- Variable (Symbol) layer ---

      var_new_out.clear();

      for &succ in &succs[i] {
        var_new_out.union_with(&var_live_in[succ]);
      }

      if var_new_out != var_live_out[i] {
        var_live_out[i].copy_from(&var_new_out);
        changed = true;
      }

      var_new_in.copy_from(&var_uses[i]);
      var_tmp.copy_from(&var_live_out[i]);
      var_tmp.difference_with(&var_defs[i]);
      var_new_in.union_with(&var_tmp);

      if var_new_in != var_live_in[i] {
        var_live_in[i].copy_from(&var_new_in);
        changed = true;
      }
    }
  }

  LivenessInfo {
    live_in,
    live_out,
    var_live_out,
    var_map,
    vid_map,
  }
}
