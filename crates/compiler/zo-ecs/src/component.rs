//! Component identity + registry.
//!
//! Each component type registered with the World gets a
//! u32 [`ComponentId`]. The registry stores per-type
//! metadata (`Layout` for size+align, drop fn for E5
//! safety, `TypeId` for idempotent registration).
//!
//! Registration is idempotent: calling `register::<T>()`
//! twice returns the same `ComponentId`.

use std::alloc::Layout;
use std::any::TypeId;
use std::collections::HashMap;

/// Stable handle for a registered component type.
/// 4 bytes; `Copy`, hashable, comparable.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, PartialOrd, Ord)]
pub struct ComponentId(u32);

impl ComponentId {
  #[inline(always)]
  pub(crate) const fn new(raw: u32) -> Self {
    Self(raw)
  }

  /// Underlying index. Useful for testing and for column
  /// keying inside an archetype.
  #[inline(always)]
  pub const fn raw(self) -> u32 {
    self.0
  }
}

/// Per-type metadata recorded at registration time. The
/// drop fn is invoked by `Column::swap_remove` (per
/// removed row) and `Column::Drop` (per live row at world
/// teardown), so non-`Copy` components don't leak.
#[derive(Clone, Copy)]
pub(crate) struct ComponentInfo {
  pub type_id: TypeId,
  pub layout: Layout,
  pub drop: unsafe fn(*mut u8),
}

/// Registry. Two-way mapping: TypeId → ComponentId for
/// idempotent registration, ComponentId → ComponentInfo
/// for storage layout lookups.
pub(crate) struct ComponentRegistry {
  by_type: HashMap<TypeId, ComponentId>,
  infos: Vec<ComponentInfo>,
}

impl ComponentRegistry {
  pub fn new() -> Self {
    Self {
      by_type: HashMap::new(),
      infos: Vec::new(),
    }
  }

  /// Idempotent register. Returns the same id on repeat
  /// calls for the same `T`. Records `Layout` and a typed
  /// drop fn produced via `std::ptr::drop_in_place::<T>`.
  pub fn register<T: 'static>(&mut self) -> ComponentId {
    let type_id = TypeId::of::<T>();

    if let Some(id) = self.by_type.get(&type_id) {
      return *id;
    }

    let info = ComponentInfo {
      type_id,
      layout: Layout::new::<T>(),
      drop: drop_fn::<T>,
    };

    let id = ComponentId::new(self.infos.len() as u32);
    self.infos.push(info);
    self.by_type.insert(type_id, id);

    id
  }

  /// Type-checked lookup — used by `EntityBuilder::with`
  /// to verify the user passed the right type for a given
  /// `ComponentId`. Returns `None` on TypeId mismatch.
  pub fn assert_type<T: 'static>(
    &self,
    id: ComponentId,
  ) -> Option<&ComponentInfo> {
    let info = self.infos.get(id.raw() as usize)?;

    if info.type_id == TypeId::of::<T>() {
      Some(info)
    } else {
      None
    }
  }

  /// Layout of a registered component. Used by archetype
  /// storage to size its column buffers.
  pub fn info(&self, id: ComponentId) -> &ComponentInfo {
    &self.infos[id.raw() as usize]
  }

  /// Look up the `ComponentId` previously registered for
  /// the given `TypeId`. Returns `None` if the type was
  /// never registered. Used by queries to translate
  /// `TypeId::of::<T>()` into the index used by archetype
  /// columns.
  pub fn id_of(&self, type_id: TypeId) -> Option<ComponentId> {
    self.by_type.get(&type_id).copied()
  }
}

/// Drops a value of type `T` through a raw pointer. Safe
/// to invoke iff the pointer addresses a valid `T` that
/// hasn't been moved out of since construction.
unsafe fn drop_fn<T>(ptr: *mut u8) {
  unsafe { std::ptr::drop_in_place(ptr as *mut T) };
}
