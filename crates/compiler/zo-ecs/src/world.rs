//! World — owns the entity allocator, the component
//! registry, and the archetype storage.
//!
//! Entity allocation: free-list over `Vec<Slot>` (E1).
//! Components: archetype-keyed `HashMap` of column-major
//! row stores (E2). Each entity's `Location` records
//! `(archetype, row)` so despawn can swap-remove the row
//! without scanning.

#[cfg(test)]
use crate::archetype::Column;
use crate::archetype::{Archetype, ArchetypeKey};
use crate::component::{ComponentId, ComponentRegistry};
use crate::entity::Entity;
use crate::query::{Query2, Query2Mut};

use std::collections::HashMap;

#[derive(Clone, Copy)]
struct Slot {
  generation: u32,
  alive: bool,
}

#[derive(Clone, Copy)]
struct Location {
  archetype: u32,
  row: u32,
}

pub struct World {
  // entity allocator (E1)
  slots: Vec<Slot>,
  free: Vec<u32>,
  alive_count: usize,

  // E2 storage
  pub(crate) registry: ComponentRegistry,
  pub(crate) archetypes: Vec<Archetype>,
  // ArchetypeKey → index into `archetypes`. The empty
  // archetype is always index 0 (pre-created).
  archetype_index: HashMap<ArchetypeKey, u32>,
  // Slot index → location of the entity currently in that
  // slot (Some when alive, None when free).
  locations: Vec<Option<Location>>,
}

impl World {
  pub fn new() -> Self {
    let mut w = Self {
      slots: Vec::new(),
      free: Vec::new(),
      alive_count: 0,
      registry: ComponentRegistry::new(),
      archetypes: Vec::new(),
      archetype_index: HashMap::new(),
      locations: Vec::new(),
    };

    // Pre-create the empty archetype as index 0.
    let empty_key = ArchetypeKey::empty();
    w.archetypes.push(Archetype::new(empty_key.clone(), &[]));
    w.archetype_index.insert(empty_key, 0);

    w
  }

  pub fn with_capacity(cap: usize) -> Self {
    let mut w = Self::new();
    w.slots.reserve(cap);
    w.locations.reserve(cap);
    w
  }

  /// Register a component type. Idempotent — repeat calls
  /// for the same `T` return the same id.
  pub fn register<T: 'static>(&mut self) -> ComponentId {
    self.registry.register::<T>()
  }

  /// Begin spawning an entity. Use `.with(...)` to attach
  /// components, then `.build()` to commit.
  pub fn spawn(&mut self) -> EntityBuilder<'_> {
    EntityBuilder {
      world: self,
      components: Vec::new(),
    }
  }

  /// Read-only 2-component query. Yields `(A, B)` per
  /// matching row across all archetypes that contain both
  /// components. Components must be `Copy` — values are
  /// moved out of the column buffer via `read_unaligned`.
  pub fn query2<A, B>(&self) -> Query2<'_, A, B>
  where
    A: Copy + 'static,
    B: Copy + 'static,
  {
    Query2::new(self)
  }

  /// Mutating 2-component query. Driven via a closure:
  /// `world.query2_mut::<A, B>().for_each_mut(|a, b| ...)`.
  /// The closure receives `&mut A, &mut B`; writes are
  /// flushed to the column buffer at the end of each row.
  pub fn query2_mut<A, B>(&mut self) -> Query2Mut<'_, A, B>
  where
    A: Copy + 'static,
    B: Copy + 'static,
  {
    Query2Mut::new(self)
  }

  /// Despawn an entity. Returns true on success. Removes
  /// the entity's row from its archetype via swap-remove
  /// and updates the displaced entity's location.
  pub fn despawn(&mut self, entity: Entity) -> bool {
    if !self.is_alive(entity) {
      return false;
    }

    let loc = self.locations[entity.index() as usize]
      .expect("alive entity must have a location");

    // Swap-remove the row from its archetype.
    let archetype = &mut self.archetypes[loc.archetype as usize];
    let displaced = archetype.swap_remove_row(loc.row);

    // If a row got moved into our slot, update its
    // entity's location.
    if let Some(moved) = displaced {
      self.locations[moved.index() as usize] = Some(loc);
    }

    // Mark the entity slot dead.
    let slot = &mut self.slots[entity.index() as usize];
    slot.alive = false;
    slot.generation = slot.generation.wrapping_add(1);

    self.locations[entity.index() as usize] = None;
    self.free.push(entity.index());
    self.alive_count -= 1;

    true
  }

  #[inline]
  pub fn is_alive(&self, entity: Entity) -> bool {
    self
      .slots
      .get(entity.index() as usize)
      .is_some_and(|s| s.alive && s.generation == entity.generation())
  }

  #[inline]
  pub fn len(&self) -> usize {
    self.alive_count
  }

  #[inline]
  pub fn is_empty(&self) -> bool {
    self.alive_count == 0
  }

  /// Number of distinct archetypes currently allocated.
  /// The empty archetype counts; minimum value is 1.
  #[inline]
  pub fn archetype_count(&self) -> usize {
    self.archetypes.len()
  }

  /// Read component `T` for `entity`. Returns `Some(value)`
  /// when the entity is alive AND its archetype has a `T`
  /// column; `None` otherwise (dead handle, never had `T`,
  /// or the type was never registered).
  ///
  /// O(1) — entity → location → archetype → column → row.
  pub fn get<T: Copy + 'static>(&self, entity: Entity) -> Option<T> {
    if !self.is_alive(entity) {
      return None;
    }

    let loc = (*self.locations.get(entity.index() as usize)?)?;
    let id = self.registry.id_of(std::any::TypeId::of::<T>())?;
    let archetype = self.archetypes.get(loc.archetype as usize)?;
    let column = archetype.columns.get(&id)?;
    let stride = column.stride();
    let offset = (loc.row as usize) * stride;

    // Storage is aligned to T's natural alignment, so a
    // plain `ptr::read` is sound. Copy bound on T means
    // we move bytes out without disturbing storage.
    let value =
      unsafe { std::ptr::read(column.as_ptr().add(offset) as *const T) };

    Some(value)
  }

  /// Overwrite component `T` for `entity`. Returns `true`
  /// when the write happened, `false` when the entity is
  /// dead, missing the component, or `T` was never
  /// registered. O(1) — same path as [`Self::get`].
  pub fn set<T: Copy + 'static>(&mut self, entity: Entity, value: T) -> bool {
    if !self.is_alive(entity) {
      return false;
    }

    let Some(loc_slot) = self.locations.get(entity.index() as usize) else {
      return false;
    };
    let Some(loc) = *loc_slot else {
      return false;
    };

    let Some(id) = self.registry.id_of(std::any::TypeId::of::<T>()) else {
      return false;
    };

    let Some(archetype) = self.archetypes.get_mut(loc.archetype as usize)
    else {
      return false;
    };

    let Some(column) = archetype.columns.get_mut(&id) else {
      return false;
    };

    let stride = column.stride();
    let offset = (loc.row as usize) * stride;

    unsafe {
      std::ptr::write(column.as_mut_ptr().add(offset) as *mut T, value);
    }

    true
  }

  // -- E2 internals ----------------------------------------

  /// Allocate an entity slot. Internal — used by builder.
  fn allocate_entity(&mut self) -> Entity {
    self.alive_count += 1;

    if let Some(index) = self.free.pop() {
      let slot = &mut self.slots[index as usize];
      slot.alive = true;

      Entity::new(index, slot.generation)
    } else {
      let index = self.slots.len() as u32;

      self.slots.push(Slot {
        generation: 0,
        alive: true,
      });
      self.locations.push(None);

      Entity::new(index, 0)
    }
  }

  /// Insert an entity into the archetype matching the
  /// given component set. Creates the archetype on first
  /// use.
  fn insert_entity(
    &mut self,
    entity: Entity,
    components: Vec<(ComponentId, Box<[u8]>)>,
  ) {
    let key = ArchetypeKey::from_unsorted(
      components.iter().map(|(id, _)| *id).collect(),
    );

    let archetype_idx = match self.archetype_index.get(&key) {
      Some(idx) => *idx,
      None => {
        let infos: Vec<_> = key
          .ids()
          .iter()
          .map(|id| (*id, self.registry.info(*id)))
          .collect();
        let archetype = Archetype::new(key.clone(), &infos);

        let idx = self.archetypes.len() as u32;
        self.archetypes.push(archetype);
        self.archetype_index.insert(key, idx);

        idx
      }
    };

    let row =
      self.archetypes[archetype_idx as usize].push_row(entity, &components);

    self.locations[entity.index() as usize] = Some(Location {
      archetype: archetype_idx,
      row,
    });
  }

  // -- Test-only introspection ----------------------------

  #[cfg(test)]
  pub(crate) fn archetype_len_for(&self, ids: &[ComponentId]) -> Option<usize> {
    let key = ArchetypeKey::from_unsorted(ids.to_vec());
    self
      .archetype_index
      .get(&key)
      .map(|idx| self.archetypes[*idx as usize].len())
  }

  #[cfg(test)]
  pub(crate) fn entity_archetype(&self, entity: Entity) -> Option<u32> {
    self
      .locations
      .get(entity.index() as usize)
      .copied()
      .flatten()
      .map(|l| l.archetype)
  }

  #[cfg(test)]
  pub(crate) fn read_component_bytes(
    &self,
    entity: Entity,
    id: ComponentId,
  ) -> Option<&[u8]> {
    let loc = (*self.locations.get(entity.index() as usize)?).as_ref()?;
    let archetype = &self.archetypes[loc.archetype as usize];
    let column: &Column = archetype.columns.get(&id)?;
    let stride = column.stride();
    let start = (loc.row as usize) * stride;
    // Safety: row is in-bounds (checked via loc), stride
    // bytes follow, and the bytes are a live value in the
    // column buffer.
    let slice =
      unsafe { std::slice::from_raw_parts(column.as_ptr().add(start), stride) };
    Some(slice)
  }
}

impl Default for World {
  fn default() -> Self {
    Self::new()
  }
}

/// Builder returned by `World::spawn`. Collects pending
/// components, then `.build()` commits the entity into
/// the archetype matching its component set.
pub struct EntityBuilder<'w> {
  world: &'w mut World,
  components: Vec<(ComponentId, Box<[u8]>)>,
}

impl EntityBuilder<'_> {
  /// Attach a component value to this entity.
  ///
  /// The runtime checks that `T` matches the type
  /// originally registered for `id`. Mismatch panics —
  /// this is a programmer error, not a recoverable
  /// runtime case.
  pub fn with<T: 'static>(mut self, id: ComponentId, value: T) -> Self {
    let info = self
      .world
      .registry
      .assert_type::<T>(id)
      .expect("ComponentId / type mismatch in EntityBuilder::with");

    let size = info.layout.size();
    let mut bytes = vec![0u8; size].into_boxed_slice();

    // Move the value's bytes into the boxed buffer.
    // ManuallyDrop suppresses T's Drop here — the bytes
    // will be dropped via the registered drop fn when the
    // archetype's row is removed (E5 wires drops; until
    // then values just leak — fine for E2 unit tests
    // since their types are Copy).
    let mut value = std::mem::ManuallyDrop::new(value);
    unsafe {
      std::ptr::copy_nonoverlapping(
        &mut *value as *mut T as *mut u8,
        bytes.as_mut_ptr(),
        size,
      );
    }

    self.components.push((id, bytes));
    self
  }

  /// Commit the entity. Allocates an entity slot, finds
  /// or creates the archetype matching the component set,
  /// appends the row, records the location.
  pub fn build(self) -> Entity {
    let entity = self.world.allocate_entity();
    self.world.insert_entity(entity, self.components);
    entity
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  // --- E1 carry-overs (now via .build()) ---------------

  #[test]
  fn spawn_returns_alive_entity() {
    let mut w = World::new();

    let e = w.spawn().build();

    assert!(w.is_alive(e));
    assert_eq!(w.len(), 1);
    assert_eq!(e.index(), 0);
    assert_eq!(e.generation(), 0);
  }

  #[test]
  fn despawn_marks_dead() {
    let mut w = World::new();
    let e = w.spawn().build();

    assert!(w.despawn(e));
    assert!(!w.is_alive(e));
    assert_eq!(w.len(), 0);
  }

  #[test]
  fn despawn_twice_is_idempotent_false() {
    let mut w = World::new();
    let e = w.spawn().build();

    assert!(w.despawn(e));
    assert!(!w.despawn(e));
  }

  #[test]
  fn stale_handle_never_alive_after_recycle() {
    let mut w = World::new();
    let e0 = w.spawn().build();
    w.despawn(e0);

    let e1 = w.spawn().build();

    assert_eq!(e0.index(), e1.index());
    assert_ne!(e0.generation(), e1.generation());

    assert!(!w.is_alive(e0));
    assert!(w.is_alive(e1));
  }

  #[test]
  fn alternating_despawn_keeps_count_correct() {
    let mut w = World::new();
    let entities: Vec<_> = (0..1000).map(|_| w.spawn().build()).collect();

    for (i, e) in entities.iter().enumerate() {
      if i % 2 == 0 {
        assert!(w.despawn(*e));
      }
    }

    assert_eq!(w.len(), 500);

    for (i, e) in entities.iter().enumerate() {
      assert_eq!(w.is_alive(*e), i % 2 == 1);
    }
  }

  // --- E2 component tests --------------------------------

  #[derive(Debug, Clone, Copy, PartialEq)]
  struct Mesh {
    id: u32,
  }
  #[derive(Debug, Clone, Copy, PartialEq)]
  struct Transform {
    x: f32,
    y: f32,
  }
  #[derive(Debug, Clone, Copy, PartialEq)]
  struct Material {
    color: u32,
  }

  #[test]
  fn register_is_idempotent() {
    let mut w = World::new();

    let a = w.register::<Mesh>();
    let b = w.register::<Mesh>();

    assert_eq!(a, b);

    let c = w.register::<Transform>();
    assert_ne!(a, c);
  }

  #[test]
  fn empty_spawn_lands_in_empty_archetype() {
    let mut w = World::new();
    let e = w.spawn().build();

    assert_eq!(w.archetype_count(), 1);
    assert_eq!(w.entity_archetype(e), Some(0));
  }

  #[test]
  fn two_components_create_one_archetype() {
    let mut w = World::new();
    let mesh_id = w.register::<Mesh>();
    let xform_id = w.register::<Transform>();

    let e = w
      .spawn()
      .with(mesh_id, Mesh { id: 7 })
      .with(xform_id, Transform { x: 1.0, y: 2.0 })
      .build();

    // Empty + (Mesh, Transform).
    assert_eq!(w.archetype_count(), 2);
    assert!(w.is_alive(e));
    assert_eq!(w.archetype_len_for(&[mesh_id, xform_id]), Some(1));
  }

  #[test]
  fn archetype_canonicalization_ignores_with_order() {
    let mut w = World::new();
    let m = w.register::<Mesh>();
    let t = w.register::<Transform>();

    let e1 = w
      .spawn()
      .with(m, Mesh { id: 1 })
      .with(t, Transform { x: 0.0, y: 0.0 })
      .build();
    let e2 = w
      .spawn()
      .with(t, Transform { x: 1.0, y: 1.0 })
      .with(m, Mesh { id: 2 })
      .build();

    assert_eq!(w.entity_archetype(e1), w.entity_archetype(e2));
    assert_eq!(w.archetype_len_for(&[m, t]), Some(2));
  }

  #[test]
  fn different_component_set_is_different_archetype() {
    let mut w = World::new();
    let m = w.register::<Mesh>();
    let t = w.register::<Transform>();
    let mat = w.register::<Material>();

    let e_mt = w
      .spawn()
      .with(m, Mesh { id: 1 })
      .with(t, Transform { x: 0.0, y: 0.0 })
      .build();
    let e_mtmat = w
      .spawn()
      .with(m, Mesh { id: 2 })
      .with(t, Transform { x: 0.0, y: 0.0 })
      .with(mat, Material { color: 0xFF })
      .build();

    assert_ne!(w.entity_archetype(e_mt), w.entity_archetype(e_mtmat));
  }

  #[test]
  fn component_bytes_round_trip() {
    let mut w = World::new();
    let m = w.register::<Mesh>();

    let e = w.spawn().with(m, Mesh { id: 0xCAFEBABE }).build();
    let bytes = w.read_component_bytes(e, m).unwrap();

    let mut got = Mesh { id: 0 };
    unsafe {
      std::ptr::copy_nonoverlapping(
        bytes.as_ptr(),
        &mut got as *mut Mesh as *mut u8,
        std::mem::size_of::<Mesh>(),
      );
    }

    assert_eq!(got.id, 0xCAFEBABE);
  }

  // --- E5: drop safety + alignment --------------------

  use std::sync::atomic::{AtomicUsize, Ordering};

  /// Test component with a non-trivial Drop. Increments a
  /// shared atomic counter every time `drop` runs — lets
  /// tests assert that `Column::swap_remove` and
  /// `Column::Drop` actually invoke the registered drop fn.
  struct DropCounter {
    counter: &'static AtomicUsize,
  }

  impl Drop for DropCounter {
    fn drop(&mut self) {
      self.counter.fetch_add(1, Ordering::SeqCst);
    }
  }

  /// 64-byte aligned component — proves the column buffer
  /// honors stricter-than-default alignment requirements.
  #[derive(Debug, Clone, Copy, PartialEq)]
  #[repr(C, align(64))]
  struct OverAligned {
    x: u64,
    y: u64,
  }

  #[test]
  fn despawn_drops_components() {
    static COUNT: AtomicUsize = AtomicUsize::new(0);
    COUNT.store(0, Ordering::SeqCst);

    let mut w = World::new();
    let dc = w.register::<DropCounter>();

    let entities: Vec<_> = (0..3)
      .map(|_| w.spawn().with(dc, DropCounter { counter: &COUNT }).build())
      .collect();

    assert_eq!(COUNT.load(Ordering::SeqCst), 0);

    // Despawn middle entity → swap_remove fires drop on
    // its row exactly once. The displaced entity's bytes
    // are byte-copied (no second drop on the source slot).
    assert!(w.despawn(entities[1]));
    assert_eq!(COUNT.load(Ordering::SeqCst), 1);

    // Despawn the rest one by one.
    assert!(w.despawn(entities[0]));
    assert_eq!(COUNT.load(Ordering::SeqCst), 2);

    assert!(w.despawn(entities[2]));
    assert_eq!(COUNT.load(Ordering::SeqCst), 3);
  }

  #[test]
  fn world_drop_runs_drops_for_live_entities() {
    static COUNT: AtomicUsize = AtomicUsize::new(0);
    COUNT.store(0, Ordering::SeqCst);

    {
      let mut w = World::new();
      let dc = w.register::<DropCounter>();
      for _ in 0..7 {
        w.spawn().with(dc, DropCounter { counter: &COUNT }).build();
      }

      assert_eq!(COUNT.load(Ordering::SeqCst), 0);
    } // World drops here.

    assert_eq!(COUNT.load(Ordering::SeqCst), 7);
  }

  #[test]
  fn world_drop_skips_already_despawned_entities() {
    // Mixed: some despawned (drop fired), some live (drop
    // fires at world drop). Total should match spawn count.
    static COUNT: AtomicUsize = AtomicUsize::new(0);
    COUNT.store(0, Ordering::SeqCst);

    {
      let mut w = World::new();
      let dc = w.register::<DropCounter>();
      let entities: Vec<_> = (0..5)
        .map(|_| w.spawn().with(dc, DropCounter { counter: &COUNT }).build())
        .collect();

      // Despawn 2 of 5.
      w.despawn(entities[1]);
      w.despawn(entities[3]);
      assert_eq!(COUNT.load(Ordering::SeqCst), 2);
    }

    // Remaining 3 dropped at world drop → total 5.
    assert_eq!(COUNT.load(Ordering::SeqCst), 5);
  }

  #[test]
  fn over_aligned_component_round_trips() {
    let mut w = World::new();
    let oa = w.register::<OverAligned>();

    let mut entities = Vec::new();
    for i in 0..16 {
      let e = w.spawn().with(oa, OverAligned { x: i, y: i * 2 }).build();
      entities.push((e, OverAligned { x: i, y: i * 2 }));
    }

    // Each row pointer must satisfy 64-byte alignment.
    for (e, expected) in &entities {
      let bytes = w.read_component_bytes(*e, oa).unwrap();
      let addr = bytes.as_ptr() as usize;
      assert_eq!(addr % 64, 0, "row pointer {addr:#x} not 64-byte aligned");

      let mut got = OverAligned { x: 0, y: 0 };
      unsafe {
        std::ptr::copy_nonoverlapping(
          bytes.as_ptr(),
          &mut got as *mut OverAligned as *mut u8,
          std::mem::size_of::<OverAligned>(),
        );
      }
      assert_eq!(got, *expected);
    }
  }

  #[test]
  fn over_aligned_query_iterates_correctly() {
    #[derive(Debug, Clone, Copy, PartialEq)]
    struct Tag {
      v: u32,
    }

    let mut w = World::new();
    let oa = w.register::<OverAligned>();
    let t = w.register::<Tag>();

    for i in 0..32u64 {
      w.spawn()
        .with(oa, OverAligned { x: i, y: i * 10 })
        .with(t, Tag { v: i as u32 })
        .build();
    }

    let mut sum = 0u64;
    for (a, _tag) in w.query2::<OverAligned, Tag>().iter() {
      sum += a.x + a.y;
    }
    // sum_{i=0..32} (i + 10i) = 11 * sum_{i=0..32} i
    //                        = 11 * (31 * 32 / 2) = 11 * 496 = 5456
    assert_eq!(sum, 5456);
  }

  // --- M3a: World::get / World::set ---------------------

  #[test]
  fn get_returns_none_when_unregistered() {
    let w = World::new();
    let e = Entity::new(0, 0);
    assert!(w.get::<Mesh>(e).is_none());
  }

  #[test]
  fn get_returns_none_for_dead_entity() {
    let mut w = World::new();
    let m = w.register::<Mesh>();
    let e = w.spawn().with(m, Mesh { id: 7 }).build();
    assert!(w.despawn(e));
    assert!(w.get::<Mesh>(e).is_none());
  }

  #[test]
  fn get_returns_none_when_component_missing() {
    let mut w = World::new();
    let m = w.register::<Mesh>();
    let _t = w.register::<Transform>();
    let e = w.spawn().with(m, Mesh { id: 1 }).build();
    // Entity has Mesh but not Transform.
    assert!(w.get::<Transform>(e).is_none());
  }

  #[test]
  fn get_round_trips_component_value() {
    let mut w = World::new();
    let m = w.register::<Mesh>();
    let t = w.register::<Transform>();
    let e = w
      .spawn()
      .with(m, Mesh { id: 0xCAFE })
      .with(t, Transform { x: 3.5, y: -7.0 })
      .build();
    assert_eq!(w.get::<Mesh>(e), Some(Mesh { id: 0xCAFE }));
    assert_eq!(w.get::<Transform>(e), Some(Transform { x: 3.5, y: -7.0 }));
  }

  #[test]
  fn set_updates_value_visible_via_get() {
    let mut w = World::new();
    let t = w.register::<Transform>();
    let e = w.spawn().with(t, Transform { x: 0.0, y: 0.0 }).build();
    assert!(w.set(e, Transform { x: 9.0, y: -3.0 }));
    assert_eq!(w.get::<Transform>(e), Some(Transform { x: 9.0, y: -3.0 }));
  }

  #[test]
  fn set_returns_false_for_dead_or_missing() {
    let mut w = World::new();
    let m = w.register::<Mesh>();
    let _t = w.register::<Transform>();
    let e = w.spawn().with(m, Mesh { id: 1 }).build();

    // Right entity, wrong component.
    assert!(!w.set(e, Transform { x: 0.0, y: 0.0 }));

    // Dead entity.
    w.despawn(e);
    assert!(!w.set(e, Mesh { id: 99 }));
  }

  #[test]
  fn set_does_not_disturb_other_columns() {
    let mut w = World::new();
    let m = w.register::<Mesh>();
    let t = w.register::<Transform>();
    let e = w
      .spawn()
      .with(m, Mesh { id: 11 })
      .with(t, Transform { x: 1.0, y: 2.0 })
      .build();
    assert!(w.set(e, Transform { x: 100.0, y: 200.0 }));
    // Mesh untouched.
    assert_eq!(w.get::<Mesh>(e), Some(Mesh { id: 11 }));
    assert_eq!(
      w.get::<Transform>(e),
      Some(Transform { x: 100.0, y: 200.0 })
    );
  }

  #[test]
  fn despawn_swap_remove_updates_locations() {
    let mut w = World::new();
    let m = w.register::<Mesh>();

    let entities: Vec<_> = (0..5)
      .map(|i| w.spawn().with(m, Mesh { id: i as u32 }).build())
      .collect();

    // Despawn the middle one. Swap-remove moves the last
    // entity into its row — its location must update to
    // the new row, and its component bytes must follow.
    assert!(w.despawn(entities[2]));

    // The originally-last entity (Mesh { id: 4 }) is now
    // at row 2 of the (Mesh,) archetype. Reading its
    // component should still yield id=4.
    let bytes = w.read_component_bytes(entities[4], m).unwrap();
    let mut got = Mesh { id: 0 };
    unsafe {
      std::ptr::copy_nonoverlapping(
        bytes.as_ptr(),
        &mut got as *mut Mesh as *mut u8,
        std::mem::size_of::<Mesh>(),
      );
    }
    assert_eq!(got.id, 4);

    assert_eq!(w.archetype_len_for(&[m]), Some(4));
  }
}
