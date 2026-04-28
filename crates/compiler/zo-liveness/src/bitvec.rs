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

  /// `self |= other`. Returns true if self changed.
  pub fn union_with(&mut self, other: &Self) -> bool {
    let mut changed = false;

    for (a, b) in self.words.iter_mut().zip(other.words.iter()) {
      let old = *a;
      *a |= *b;
      changed |= *a != old;
    }

    changed
  }

  /// `self &= !other`.
  pub fn difference_with(&mut self, other: &Self) {
    for (a, b) in self.words.iter_mut().zip(other.words.iter()) {
      *a &= !*b;
    }
  }

  /// `self := other`. In-place copy that reuses the
  /// existing word storage. Caller guarantees both
  /// bitvecs have the same `nbits` (same word length) —
  /// if `other` is shorter, the tail words of `self` are
  /// zeroed; if longer, the surplus is discarded. Used by
  /// the liveness fixed-point loop to recycle hoisted
  /// scratch buffers instead of cloning a fresh `BitVec`
  /// per instruction per round.
  pub fn copy_from(&mut self, other: &Self) {
    let n = self.words.len().min(other.words.len());

    self.words[..n].copy_from_slice(&other.words[..n]);

    for w in &mut self.words[n..] {
      *w = 0;
    }
  }

  /// Zero every bit in place. Capacity retained.
  pub fn clear(&mut self) {
    for w in self.words.iter_mut() {
      *w = 0;
    }
  }

  /// Returns true if no bits are set.
  pub fn is_empty(&self) -> bool {
    self.words.iter().all(|&w| w == 0)
  }
}
