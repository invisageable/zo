//! Data-oriented lookup tables keyed by dense
//! `u32`-backed ids ([`Symbol`], `ValueId`, `TyId`,
//! `LabelId`, `InferVarId`, ...).
//!
//! The compiler's id types are allocated sequentially
//! from `0` by their owning crate. The DOD-correct
//! `Id → V` lookup is `Vec<V>` indexed by
//! `id.to_u32() as usize`, with a sentinel value
//! encoding "absent". This module hosts that shape behind
//! typed wrappers so call sites read `map.get(sym)`
//! instead of `vec[sym.as_u32() as usize]` with sentinel
//! branching inline.
//!
//! Lives in `zo-interner` because every crate that has
//! ids already depends on `zo-interner` for `Symbol`;
//! folding the lookup tables in here avoids a parallel
//! crate that would just re-export the same dep edge.
//!
//! - [`DenseId`] — implemented by id types in their owning
//!   crates ([`Symbol`] here, `ValueId` in `zo-value`,
//!   etc.).
//! - [`Sentinel`] — implemented by value types that
//!   reserve one bit pattern for "absent".
//! - [`DenseMap`] — typed `Id → V` lookup.
//! - [`DenseSet`] — typed `Id` membership, bitset.
//! - [`ScopedDenseMap`] — adds a flat shadow stack so
//!   pushing a binding can be rolled back at scope exit.

use std::marker::PhantomData;

use crate::symbol::Symbol;

/// A dense, small-`u32` id usable as a [`DenseMap`] key.
///
/// "Dense" means consecutive — ids are allocated by their
/// source (interner, SIR builder, ty-checker) starting
/// at `0` and incremented one at a time. The lookup
/// tables in this module exploit that — they are flat
/// `Vec`s indexed by `id.to_u32() as usize`, sized to the
/// max id seen so far. A non-dense id space (e.g. random
/// `u32` values) would blow the table out to several GB.
pub trait DenseId: Copy + Eq {
  fn from_u32(id: u32) -> Self;
  fn to_u32(self) -> u32;
}

impl DenseId for u32 {
  #[inline]
  fn from_u32(id: u32) -> Self {
    id
  }

  #[inline]
  fn to_u32(self) -> u32 {
    self
  }
}

impl DenseId for Symbol {
  #[inline]
  fn from_u32(id: u32) -> Self {
    Symbol(id)
  }

  #[inline]
  fn to_u32(self) -> u32 {
    self.0
  }
}

/// A value type with a reserved "absent" bit pattern.
///
/// `DenseMap` stores `Vec<V>` directly (no `Option<V>`
/// overhead) and reads `V == V::ABSENT` as "no entry".
/// Most uses set `ABSENT = Self(u32::MAX)` for index
/// newtypes; ids never reach `u32::MAX` in any realistic
/// program.
pub trait Sentinel: Copy + Eq {
  const ABSENT: Self;
}

impl Sentinel for u32 {
  const ABSENT: u32 = u32::MAX;
}

const BITS_PER_WORD: usize = 64;

/// Sparse map keyed by a [`DenseId`] (`Symbol`,
/// `ValueId`, `TyId`, ...).
///
/// One direct memory load per lookup, no hashing. Grows
/// on insert; the underlying `Vec` is sized to the
/// largest key seen plus one. Memory cost is `O(max_id)`
/// — well under a megabyte for compiler workloads.
pub struct DenseMap<K: DenseId, V: Sentinel> {
  slots: Vec<V>,
  _key: PhantomData<fn(K) -> K>,
}

impl<K: DenseId, V: Sentinel> Default for DenseMap<K, V> {
  fn default() -> Self {
    Self::new()
  }
}

impl<K: DenseId, V: Sentinel> DenseMap<K, V> {
  pub fn new() -> Self {
    Self {
      slots: Vec::new(),
      _key: PhantomData,
    }
  }

  pub fn with_capacity(cap: usize) -> Self {
    Self {
      slots: Vec::with_capacity(cap),
      _key: PhantomData,
    }
  }

  /// Insert `value` at `key`. Grows the underlying `Vec`
  /// to fit if necessary.
  pub fn insert(&mut self, key: K, value: V) {
    let i = key.to_u32() as usize;

    if self.slots.len() <= i {
      self.slots.resize(i + 1, V::ABSENT);
    }

    self.slots[i] = value;
  }

  /// `Some(value)` if a non-`ABSENT` value is stored at
  /// `key`, `None` otherwise.
  pub fn get(&self, key: K) -> Option<V> {
    let i = key.to_u32() as usize;

    match self.slots.get(i).copied() {
      Some(v) if v != V::ABSENT => Some(v),
      _ => None,
    }
  }

  /// `true` iff a non-`ABSENT` value is stored at `key`.
  pub fn contains(&self, key: K) -> bool {
    self.get(key).is_some()
  }

  /// Remove the entry at `key`. The underlying `Vec` is
  /// not shrunk.
  pub fn remove(&mut self, key: K) {
    let i = key.to_u32() as usize;

    if i < self.slots.len() {
      self.slots[i] = V::ABSENT;
    }
  }

  /// Reset every slot to `V::ABSENT`. Capacity is
  /// retained.
  pub fn clear(&mut self) {
    for slot in self.slots.iter_mut() {
      *slot = V::ABSENT;
    }
  }

  /// Iterate `(key, value)` pairs for every non-`ABSENT`
  /// slot, in ascending key order.
  pub fn iter(&self) -> impl Iterator<Item = (K, V)> + '_ {
    self.slots.iter().enumerate().filter_map(|(i, &v)| {
      if v == V::ABSENT {
        None
      } else {
        Some((K::from_u32(i as u32), v))
      }
    })
  }
}

/// Sparse set of dense ids, encoded as a bitset.
///
/// One bit per id; `O(max_id / 8)` bytes. Sequential
/// iteration is friendly to the prefetcher; insert /
/// remove / contains are all branchless.
pub struct DenseSet<K: DenseId> {
  words: Vec<u64>,
  _key: PhantomData<fn(K) -> K>,
}

impl<K: DenseId> Default for DenseSet<K> {
  fn default() -> Self {
    Self::new()
  }
}

impl<K: DenseId> DenseSet<K> {
  pub fn new() -> Self {
    Self {
      words: Vec::new(),
      _key: PhantomData,
    }
  }

  pub fn with_capacity(max_id: usize) -> Self {
    Self {
      words: Vec::with_capacity((max_id / BITS_PER_WORD) + 1),
      _key: PhantomData,
    }
  }

  /// Add `key` to the set. Grows the underlying bitset
  /// if needed.
  pub fn insert(&mut self, key: K) {
    let i = key.to_u32() as usize;
    let word_idx = i / BITS_PER_WORD;
    let bit = i % BITS_PER_WORD;

    if self.words.len() <= word_idx {
      self.words.resize(word_idx + 1, 0);
    }

    self.words[word_idx] |= 1u64 << bit;
  }

  /// Remove `key`. No-op if absent.
  pub fn remove(&mut self, key: K) {
    let i = key.to_u32() as usize;
    let word_idx = i / BITS_PER_WORD;
    let bit = i % BITS_PER_WORD;

    if let Some(w) = self.words.get_mut(word_idx) {
      *w &= !(1u64 << bit);
    }
  }

  /// `true` iff `key` is in the set.
  pub fn contains(&self, key: K) -> bool {
    let i = key.to_u32() as usize;
    let word_idx = i / BITS_PER_WORD;
    let bit = i % BITS_PER_WORD;

    self
      .words
      .get(word_idx)
      .is_some_and(|w| (w >> bit) & 1 == 1)
  }

  /// Reset every bit. Capacity is retained.
  pub fn clear(&mut self) {
    for w in self.words.iter_mut() {
      *w = 0;
    }
  }
}

/// Opaque marker for a save-stack position. Returned by
/// [`ScopedDenseMap::checkpoint`] and consumed by
/// [`ScopedDenseMap::rollback_to`].
#[derive(Copy, Clone, Debug)]
pub struct ScopeMark(pub u32);

/// `Id → V` map with a flat shadow-stack. Pushing a
/// binding records `(id, prev_value)` on the stack; on
/// scope pop, [`ScopedDenseMap::rollback_to`] walks the
/// stack backwards and restores each prior value.
/// Cache-linear, no per-id `Vec` of stacks.
pub struct ScopedDenseMap<K: DenseId, V: Sentinel> {
  current: DenseMap<K, V>,
  save: Vec<(K, V)>,
}

impl<K: DenseId, V: Sentinel> Default for ScopedDenseMap<K, V> {
  fn default() -> Self {
    Self::new()
  }
}

impl<K: DenseId, V: Sentinel> ScopedDenseMap<K, V> {
  pub fn new() -> Self {
    Self {
      current: DenseMap::new(),
      save: Vec::new(),
    }
  }

  /// Push `(key, value)`, saving the prior value (or
  /// `V::ABSENT`) on the shadow stack.
  pub fn push(&mut self, key: K, value: V) {
    let prev = self.current.get(key).unwrap_or(V::ABSENT);

    self.save.push((key, prev));
    self.current.insert(key, value);
  }

  /// `Some(value)` for the innermost binding of `key`,
  /// `None` if no binding is live.
  pub fn get(&self, key: K) -> Option<V> {
    self.current.get(key)
  }

  /// `true` iff `key` has a live binding.
  pub fn contains(&self, key: K) -> bool {
    self.current.contains(key)
  }

  /// Snapshot the current shadow-stack length.
  pub fn checkpoint(&self) -> ScopeMark {
    ScopeMark(self.save.len() as u32)
  }

  /// Roll the shadow stack back to `mark`, restoring
  /// every prior `(key, value)` written since the
  /// matching `checkpoint`.
  pub fn rollback_to(&mut self, mark: ScopeMark) {
    let target = mark.0 as usize;

    while self.save.len() > target {
      let (k, prev) = self.save.pop().expect("checkpointed");

      if prev == V::ABSENT {
        self.current.remove(k);
      } else {
        self.current.insert(k, prev);
      }
    }
  }

  /// Reset to an empty map. Capacity is retained.
  pub fn clear(&mut self) {
    self.save.clear();
    self.current.clear();
  }
}
