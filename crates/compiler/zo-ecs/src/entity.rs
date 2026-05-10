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
}
