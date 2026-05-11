//! Entity handle: `(index, generation)` pair.
//!
//! `index` is the slot the entity occupies in [`World`].
//! `generation` bumps on every despawn — a stale handle to
//! a recycled slot fails its generation check, so
//! use-after-despawn returns false from `is_alive` instead
//! of silently hitting another entity.
//!
//! 8 bytes, `Copy`, hashable. Safe to pass through systems
//! without lifetime annotations.

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Entity {
  index: u32,
  generation: u32,
}

impl Entity {
  /// Constructor — crate-internal so user code can only
  /// obtain entities via `World::spawn`.
  #[inline(always)]
  pub(crate) const fn new(index: u32, generation: u32) -> Self {
    Self { index, generation }
  }

  /// Slot index this entity occupies.
  #[inline(always)]
  pub const fn index(self) -> u32 {
    self.index
  }

  /// Generation tag — bumps on each despawn of this slot.
  #[inline(always)]
  pub const fn generation(self) -> u32 {
    self.generation
  }

  /// Pack `(index, generation)` into a single 64-bit value
  /// for FFI handles. Inverse of [`Self::from_bits`]. Stable
  /// layout: low 32 bits = `index`, high 32 bits = `generation`.
  #[inline(always)]
  pub const fn to_bits(self) -> u64 {
    ((self.generation as u64) << 32) | (self.index as u64)
  }

  /// Reconstruct an [`Entity`] from a 64-bit handle produced
  /// by [`Self::to_bits`]. Inverse encoding: low 32 bits =
  /// `index`, high 32 bits = `generation`.
  ///
  /// Sentinel: `0` always decodes to a generation-0 entity at
  /// slot 0. Callers can use that as a "null handle" by
  /// guarding with [`crate::World::is_alive`] before
  /// dereferencing.
  #[inline(always)]
  pub const fn from_bits(bits: u64) -> Self {
    Self {
      index: bits as u32,
      generation: (bits >> 32) as u32,
    }
  }
}
