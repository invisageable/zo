use zo_sir::Insn;
use zo_value::ValueId;

use rustc_hash::FxHashMap as HashMap;

/// Compact bitvector for liveness sets.
#[derive(Clone)]
pub struct BitVec {
  words: Vec<u64>,
}

impl PartialEq for BitVec {
  fn eq(&self, other: &Self) -> bool {
    self.words == other.words
  }
}

impl BitVec {
  pub fn new(bits: usize) -> Self {
    Self {
      words: vec![0; bits.div_ceil(64)],
    }
  }

  #[inline]
  pub fn set(&mut self, bit: usize) {
    let word = bit / 64;

    if word < self.words.len() {
      self.words[word] |= 1u64 << (bit % 64);
    }
  }

  #[inline]
  pub fn test(&self, bit: usize) -> bool {
    let word = bit / 64;

    word < self.words.len() && self.words[word] & (1u64 << (bit % 64)) != 0
  }

  /// self |= other. Returns true if self changed.
  pub fn union_with(&mut self, other: &Self) -> bool {
    let mut changed = false;

    for (a, b) in self.words.iter_mut().zip(other.words.iter()) {
      let old = *a;
      *a |= *b;
      changed |= *a != old;
    }

    changed
  }

  /// self &= !other.
  pub fn difference_with(&mut self, other: &Self) {
    for (a, b) in self.words.iter_mut().zip(other.words.iter()) {
      *a &= !*b;
    }
  }
}

/// Liveness analysis result for a function body.
pub struct LivenessInfo {
  /// Per-instruction (local index) live-in sets.
  pub live_in: Vec<BitVec>,
  /// Per-instruction (local index) live-out sets.
  pub live_out: Vec<BitVec>,
}

/// Backward bitvector liveness analysis over a function body.
///
/// `insns[start..end]` is the function's instruction range.
/// `value_ids[i]` is the ValueId defined at instruction i.
/// `num_values` is the total number of ValueIds in the program
/// (used to size bitvectors).
pub fn analyze(
  insns: &[Insn],
  start: usize,
  end: usize,
  value_ids: &[Option<ValueId>],
  num_values: u32,
) -> LivenessInfo {
  let n = end - start;
  let nbits = num_values as usize;

  // --- defs and uses per local instruction ---

  let mut defs = vec![BitVec::new(nbits); n];
  let mut uses = vec![BitVec::new(nbits); n];

  for i in 0..n {
    let gi = start + i;

    if let Some(vid) = value_ids[gi] {
      defs[i].set(vid.0 as usize);
    }

    for u in crate::insn_uses(&insns[gi]) {
      if u.0 != u32::MAX {
        uses[i].set(u.0 as usize);
      }
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
      Insn::Return { .. } => {
        // No successors.
      }
      _ => {
        if i + 1 < n {
          s.push(i + 1);
        }
      }
    }

    succs.push(s);
  }

  // --- fixed-point iteration ---

  let mut live_in = vec![BitVec::new(nbits); n];
  let mut live_out = vec![BitVec::new(nbits); n];

  let mut changed = true;

  while changed {
    changed = false;

    for i in (0..n).rev() {
      // live_out[i] = ∪ live_in[successors of i]
      let mut new_out = BitVec::new(nbits);

      for &succ in &succs[i] {
        new_out.union_with(&live_in[succ]);
      }

      if new_out != live_out[i] {
        live_out[i] = new_out;
        changed = true;
      }

      // live_in[i] = uses[i] ∪ (live_out[i] \ defs[i])
      let mut new_in = uses[i].clone();
      let mut out_minus_def = live_out[i].clone();

      out_minus_def.difference_with(&defs[i]);
      new_in.union_with(&out_minus_def);

      if new_in != live_in[i] {
        live_in[i] = new_in;
        changed = true;
      }
    }
  }

  LivenessInfo { live_in, live_out }
}
