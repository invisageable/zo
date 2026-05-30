/// Compact bitvector for liveness sets.
///
/// Two-state layout — inline u64 for the small case
/// (`nbits ≤ 64`) and heap-backed `Vec<u64>` for larger.
/// The inline variant carries no allocation and turns
/// every union / diff / copy in the fixed-point loop
/// into a single u64 instruction; LLVM collapses the
/// match dispatch in hot loops where every BitVec in
/// the function shares the same variant.
///
/// Profile on 99-bottles showed liveness `BitVec::new`
/// dominating codegen at 47%. Most functions reference
/// well under 64 ValueIds — packing those into the
/// Inline variant skips both `malloc` and the `Vec<u64>`
/// indirection on every fixed-point iteration.
#[derive(Clone)]
pub enum BitVec {
  /// Up to 64 bits packed into a single word — no heap
  /// allocation, no pointer chase.
  Inline(u64),
  /// More than 64 bits — heap-backed `Vec<u64>` words,
  /// little-endian within each word (`bit i` lives at
  /// `words[i / 64] >> (i % 64) & 1`).
  Heap(Vec<u64>),
}

impl PartialEq for BitVec {
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (Self::Inline(a), Self::Inline(b)) => a == b,
      (Self::Heap(a), Self::Heap(b)) => a == b,
      // Mixed variants would imply two BitVecs created
      // from different `nbits`; not produced by
      // `liveness::analyze` which sizes every vector
      // from the same `nbits` per call.
      _ => false,
    }
  }
}

/// Low-`n`-bits-set mask, for `n` in `0..=64`. `1 << 64`
/// overflows, so the full word is its own case.
#[inline]
fn mask(n: usize) -> u64 {
  if n >= 64 { u64::MAX } else { (1u64 << n) - 1 }
}

impl BitVec {
  /// Allocates a zero-initialized bitvector with capacity
  /// for `bits` bits.
  pub fn new(bits: usize) -> Self {
    if bits <= 64 {
      Self::Inline(0)
    } else {
      Self::Heap(vec![0; bits.div_ceil(64)])
    }
  }

  /// Allocates a bitvector with exactly bits `[0, bits)` set
  /// — the universe (top) for must-analysis. The unused high
  /// bits of the last word stay zero so two `new_full`s of
  /// the same size compare equal and `intersect_with` never
  /// leaks stale high bits into the result.
  pub fn new_full(bits: usize) -> Self {
    if bits <= 64 {
      Self::Inline(mask(bits))
    } else {
      let nwords = bits.div_ceil(64);
      let mut words = vec![u64::MAX; nwords];
      let rem = bits % 64;

      if rem != 0 {
        words[nwords - 1] = mask(rem);
      }

      Self::Heap(words)
    }
  }

  #[inline]
  pub fn set(&mut self, bit: usize) {
    match self {
      Self::Inline(w) => {
        if bit < 64 {
          *w |= 1u64 << bit;
        }
      }
      Self::Heap(words) => {
        let i = bit / 64;

        if i < words.len() {
          words[i] |= 1u64 << (bit % 64);
        }
      }
    }
  }

  #[inline]
  pub fn unset(&mut self, bit: usize) {
    match self {
      Self::Inline(w) => {
        if bit < 64 {
          *w &= !(1u64 << bit);
        }
      }
      Self::Heap(words) => {
        let i = bit / 64;

        if i < words.len() {
          words[i] &= !(1u64 << (bit % 64));
        }
      }
    }
  }

  #[inline]
  pub fn test(&self, bit: usize) -> bool {
    match self {
      Self::Inline(w) => bit < 64 && (*w >> bit) & 1 == 1,
      Self::Heap(words) => {
        let i = bit / 64;

        i < words.len() && (words[i] >> (bit % 64)) & 1 == 1
      }
    }
  }

  /// `self |= other`. Returns true if self changed.
  pub fn union_with(&mut self, other: &Self) -> bool {
    match (self, other) {
      (Self::Inline(a), Self::Inline(b)) => {
        let old = *a;

        *a |= *b;

        *a != old
      }
      (Self::Heap(a), Self::Heap(b)) => {
        let mut changed = false;

        for (x, y) in a.iter_mut().zip(b.iter()) {
          let old = *x;

          *x |= *y;
          changed |= *x != old;
        }

        changed
      }
      _ => unreachable!("mismatched BitVec variants"),
    }
  }

  /// `self &= other`. Returns true if self changed. The meet
  /// for the must-be-moved (intersection) dataflow, dual to
  /// `union_with`.
  pub fn intersect_with(&mut self, other: &Self) -> bool {
    match (self, other) {
      (Self::Inline(a), Self::Inline(b)) => {
        let old = *a;

        *a &= *b;

        *a != old
      }
      (Self::Heap(a), Self::Heap(b)) => {
        let mut changed = false;

        for (x, y) in a.iter_mut().zip(b.iter()) {
          let old = *x;

          *x &= *y;
          changed |= *x != old;
        }

        changed
      }
      _ => unreachable!("mismatched BitVec variants"),
    }
  }

  /// `self &= !other`.
  pub fn difference_with(&mut self, other: &Self) {
    match (self, other) {
      (Self::Inline(a), Self::Inline(b)) => *a &= !*b,
      (Self::Heap(a), Self::Heap(b)) => {
        for (x, y) in a.iter_mut().zip(b.iter()) {
          *x &= !*y;
        }
      }
      _ => unreachable!("mismatched BitVec variants"),
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
    match (self, other) {
      (Self::Inline(a), Self::Inline(b)) => *a = *b,
      (Self::Heap(a), Self::Heap(b)) => {
        let n = a.len().min(b.len());

        a[..n].copy_from_slice(&b[..n]);

        for w in &mut a[n..] {
          *w = 0;
        }
      }
      _ => unreachable!("mismatched BitVec variants"),
    }
  }

  /// Zero every bit in place. Capacity retained.
  pub fn clear(&mut self) {
    match self {
      Self::Inline(w) => *w = 0,
      Self::Heap(words) => {
        for w in words.iter_mut() {
          *w = 0;
        }
      }
    }
  }

  /// Returns true if no bits are set.
  pub fn is_empty(&self) -> bool {
    match self {
      Self::Inline(w) => *w == 0,
      Self::Heap(words) => words.iter().all(|&w| w == 0),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn new_full_sets_exactly_the_valid_bits() {
    // Inline: every bit in range set, none out of range.
    let v = BitVec::new_full(3);
    assert!(v.test(0) && v.test(1) && v.test(2));
    assert!(!v.test(3));

    // The boundary word stays exactly `bits` wide so two
    // `new_full`s of the same size compare equal.
    assert!(BitVec::new_full(3) == BitVec::new_full(3));

    // Heap: the partial last word is masked to the valid bits.
    let h = BitVec::new_full(70);
    assert!(h.test(69));
    assert!(!h.test(70));
    assert!(BitVec::new_full(70) == BitVec::new_full(70));
  }

  #[test]
  fn intersect_is_the_meet_dual_of_union() {
    // Inline.
    let mut a = BitVec::new(8);
    a.set(1);
    a.set(2);
    let mut b = BitVec::new(8);
    b.set(2);
    b.set(3);

    let changed = a.intersect_with(&b);
    assert!(changed);
    assert!(!a.test(1) && a.test(2) && !a.test(3));

    // Intersecting with the universe is a no-op.
    let mut c = BitVec::new(8);
    c.set(5);
    assert!(!c.intersect_with(&BitVec::new_full(8)));
    assert!(c.test(5));

    // Heap path (> 64 bits).
    let mut x = BitVec::new_full(100);
    let mut y = BitVec::new(100);
    y.set(70);
    x.intersect_with(&y);
    assert!(x.test(70));
    assert!(!x.test(0) && !x.test(99));
  }
}
