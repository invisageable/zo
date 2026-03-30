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

  /// Returns true if no bits are set.
  pub fn is_empty(&self) -> bool {
    self.words.iter().all(|&w| w == 0)
  }
}
