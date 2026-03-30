//! Backward bitvector liveness analysis.

use crate::bitvec::BitVec;
use crate::insn::{insn_uses, insn_var_def, insn_var_use};

use zo_interner::Symbol;
use zo_sir::Insn;
use zo_value::ValueId;

use rustc_hash::FxHashMap as HashMap;

/// Liveness analysis result for a function body.
pub struct LivenessInfo {
  /// Per-instruction (local index) live-in sets (ValueId).
  pub live_in: Vec<BitVec>,
  /// Per-instruction (local index) live-out sets (ValueId).
  pub live_out: Vec<BitVec>,
  /// Per-instruction (local index) live-out sets for named
  /// variables (Symbols). Bit index = var_map[symbol].
  pub var_live_out: Vec<BitVec>,
  /// Maps Symbol → bit index in var bitvectors.
  pub var_map: HashMap<Symbol, usize>,
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
  num_values: u32,
) -> LivenessInfo {
  let n = end - start;
  let nbits = num_values as usize;

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

  // --- defs and uses per local instruction ---

  let mut defs = vec![BitVec::new(nbits); n];
  let mut uses = vec![BitVec::new(nbits); n];
  let mut var_defs = vec![BitVec::new(nvars); n];
  let mut var_uses = vec![BitVec::new(nvars); n];

  for i in 0..n {
    let gi = start + i;

    // ValueId defs/uses.
    if let Some(vid) = value_ids[gi] {
      defs[i].set(vid.0 as usize);
    }

    for u in insn_uses(&insns[gi]) {
      if u.0 != u32::MAX {
        uses[i].set(u.0 as usize);
      }
    }

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

  let mut changed = true;

  while changed {
    changed = false;

    for i in (0..n).rev() {
      // --- ValueId layer ---

      let mut new_out = BitVec::new(nbits);

      for &succ in &succs[i] {
        new_out.union_with(&live_in[succ]);
      }

      if new_out != live_out[i] {
        live_out[i] = new_out;
        changed = true;
      }

      let mut new_in = uses[i].clone();
      let mut out_minus_def = live_out[i].clone();

      out_minus_def.difference_with(&defs[i]);
      new_in.union_with(&out_minus_def);

      if new_in != live_in[i] {
        live_in[i] = new_in;
        changed = true;
      }

      // --- Variable (Symbol) layer ---

      let mut var_new_out = BitVec::new(nvars);

      for &succ in &succs[i] {
        var_new_out.union_with(&var_live_in[succ]);
      }

      if var_new_out != var_live_out[i] {
        var_live_out[i] = var_new_out;
        changed = true;
      }

      let mut var_new_in = var_uses[i].clone();
      let mut var_out_minus_def = var_live_out[i].clone();

      var_out_minus_def.difference_with(&var_defs[i]);
      var_new_in.union_with(&var_out_minus_def);

      if var_new_in != var_live_in[i] {
        var_live_in[i] = var_new_in;
        changed = true;
      }
    }
  }

  LivenessInfo {
    live_in,
    live_out,
    var_live_out,
    var_map,
  }
}
