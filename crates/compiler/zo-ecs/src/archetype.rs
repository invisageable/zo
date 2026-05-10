//! Archetype storage — one archetype per unique component
//! set. Entities sharing the same components live in the
//! same archetype with column-major arrays.
//!
//! Layout per archetype:
//!   - `entities: Vec<Entity>` — row index → Entity
//!   - `columns: HashMap<ComponentId, Column>` — one
//!     contiguous buffer per component; row N's bytes
//!     start at `N * row_layout.size()`.
//!
//! Each `Column` owns a raw `std::alloc::alloc`-backed
//! buffer aligned to the component's natural alignment, so
//! field reads don't need `read_unaligned` for correctness
//! (and queries can hand out genuine `&mut T` references
//! when E6 needs them).
//!
//! Despawn is swap-remove: O(1) per row. The removed row
//! is dropped via the registered `drop_fn`; the displaced
//! row's bytes are byte-copied over the hole. The
//! displaced entity's `Location.row` shifts — the caller
//! (World) must update it.

use crate::component::{ComponentId, ComponentInfo};
use crate::entity::Entity;

use std::alloc::Layout;
use std::collections::HashMap;
use std::ptr::NonNull;

/// Sorted, deduplicated `ComponentId` list — canonical key
/// for an archetype. Two entities with the same component
/// set hash and compare equal regardless of insert order.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub(crate) struct ArchetypeKey(Vec<ComponentId>);

impl ArchetypeKey {
  /// Build from an unsorted set; sorts + dedups in place.
  pub fn from_unsorted(mut ids: Vec<ComponentId>) -> Self {
    ids.sort_unstable();
    ids.dedup();
    Self(ids)
  }

  pub fn empty() -> Self {
    Self(Vec::new())
  }

  pub fn ids(&self) -> &[ComponentId] {
    &self.0
  }
}

/// One archetype: a packed set of rows, each row being a
/// tuple of component values.
pub(crate) struct Archetype {
  // Stored for archetype-graph navigation in future query
  // arities (iter all archetypes that contain a given
  // subset).
  #[allow(dead_code)]
  pub key: ArchetypeKey,
  pub entities: Vec<Entity>,
  pub columns: HashMap<ComponentId, Column>,
}

/// Owns a raw aligned allocation holding `len` rows of
/// `row_layout.size()` bytes each. Initial state is a
/// dangling pointer at align — push() lazily allocates on
/// first row.
pub(crate) struct Column {
  ptr: NonNull<u8>,
  len: usize,
  cap: usize,
  row_layout: Layout,
  /// Per-component drop fn from the registry. Run on the
  /// removed row in `swap_remove`, and on every live row
  /// in `Drop`.
  drop_fn: unsafe fn(*mut u8),
}

impl Column {
  pub fn new(row_layout: Layout, drop_fn: unsafe fn(*mut u8)) -> Self {
    // Dangling at align: aligned (align is power of two so
    // `align as *mut u8` has its low log2(align) bits
    // clear), nonzero, never dereferenced while cap == 0.
    let dangling = row_layout.align() as *mut u8;
    Self {
      ptr: unsafe { NonNull::new_unchecked(dangling) },
      len: 0,
      cap: 0,
      row_layout,
      drop_fn,
    }
  }

  // `len()` lives on Archetype.entities; Column.len is
  // tracked internally for grow/swap_remove and is not
  // exposed (reserved for future arity-N query iteration).
  #[allow(dead_code)]
  #[inline]
  pub fn len(&self) -> usize {
    self.len
  }

  #[inline]
  pub fn stride(&self) -> usize {
    self.row_layout.size()
  }

  #[inline]
  pub fn as_ptr(&self) -> *const u8 {
    self.ptr.as_ptr()
  }

  #[inline]
  pub fn as_mut_ptr(&mut self) -> *mut u8 {
    self.ptr.as_ptr()
  }

  /// Append one row's worth of bytes. Caller guarantees
  /// `bytes.len() == self.stride()` and that the bytes
  /// represent a valid value of the component type.
  pub fn push_bytes(&mut self, bytes: &[u8]) {
    debug_assert_eq!(bytes.len(), self.stride());

    if self.len == self.cap {
      self.grow();
    }

    let stride = self.stride();
    if stride > 0 {
      unsafe {
        std::ptr::copy_nonoverlapping(
          bytes.as_ptr(),
          self.ptr.as_ptr().add(self.len * stride),
          stride,
        );
      }
    }

    self.len += 1;
  }

  /// Drop the value at `row`, then byte-copy the last
  /// row's value into the hole (if `row` wasn't already
  /// the last). `len` decrements.
  pub fn swap_remove(&mut self, row: usize) {
    debug_assert!(row < self.len);

    let stride = self.stride();
    let last = self.len - 1;

    unsafe {
      let row_ptr = self.ptr.as_ptr().add(row * stride);
      (self.drop_fn)(row_ptr);

      if row != last {
        let last_ptr = self.ptr.as_ptr().add(last * stride);
        std::ptr::copy_nonoverlapping(last_ptr, row_ptr, stride);
      }
    }

    self.len -= 1;
  }

  fn grow(&mut self) {
    // ZSTs: cap stays 0, len just bumps. No allocation.
    if self.row_layout.size() == 0 {
      self.cap = usize::MAX;
      return;
    }

    let new_cap = if self.cap == 0 { 4 } else { self.cap * 2 };
    let new_size = new_cap
      .checked_mul(self.row_layout.size())
      .expect("column capacity overflow");
    let new_layout = Layout::from_size_align(new_size, self.row_layout.align())
      .expect("alignment is a power of two");

    let new_ptr = unsafe {
      if self.cap == 0 {
        std::alloc::alloc(new_layout)
      } else {
        let old_size = self.cap * self.row_layout.size();
        let old_layout =
          Layout::from_size_align_unchecked(old_size, self.row_layout.align());
        std::alloc::realloc(self.ptr.as_ptr(), old_layout, new_size)
      }
    };

    self.ptr = NonNull::new(new_ptr)
      .unwrap_or_else(|| std::alloc::handle_alloc_error(new_layout));
    self.cap = new_cap;
  }
}

impl Drop for Column {
  fn drop(&mut self) {
    let stride = self.stride();

    // Drop every live row first.
    for i in 0..self.len {
      unsafe {
        let row_ptr = self.ptr.as_ptr().add(i * stride);
        (self.drop_fn)(row_ptr);
      }
    }

    // Free the backing allocation if we ever grew.
    if self.cap > 0 && stride > 0 {
      let layout = unsafe {
        Layout::from_size_align_unchecked(
          self.cap * stride,
          self.row_layout.align(),
        )
      };
      unsafe { std::alloc::dealloc(self.ptr.as_ptr(), layout) };
    }
  }
}

impl Archetype {
  /// Empty archetype with one Column per component, each
  /// at zero rows. Caller passes per-component info from
  /// the registry.
  pub fn new(
    key: ArchetypeKey,
    infos: &[(ComponentId, &ComponentInfo)],
  ) -> Self {
    let mut columns = HashMap::with_capacity(infos.len());

    for (id, info) in infos {
      columns.insert(*id, Column::new(info.layout, info.drop));
    }

    Self {
      key,
      entities: Vec::new(),
      columns,
    }
  }

  /// Append a row. Each `bytes` slice's len must equal the
  /// matching column's stride; caller ensures the bytes
  /// were produced from a `T` whose Layout matches the
  /// registered one.
  ///
  /// Returns the new row index.
  pub fn push_row(
    &mut self,
    entity: Entity,
    components: &[(ComponentId, Box<[u8]>)],
  ) -> u32 {
    let row = self.entities.len() as u32;
    self.entities.push(entity);

    for (id, bytes) in components {
      let column = self.columns.get_mut(id).expect("column missing");
      column.push_bytes(bytes);
    }

    row
  }

  /// Swap-remove a row. Drops the removed row's component
  /// values via each column's registered drop fn, then
  /// byte-copies the last row over the hole.
  ///
  /// Returns the entity that USED to be at the last index
  /// and is now at `row` (if any) — the caller must update
  /// that entity's `Location.row`. `None` when removing
  /// the last row.
  pub fn swap_remove_row(&mut self, row: u32) -> Option<Entity> {
    let last_row = (self.entities.len() - 1) as u32;
    let row_idx = row as usize;

    self.entities.swap_remove(row_idx);

    for column in self.columns.values_mut() {
      column.swap_remove(row_idx);
    }

    if row == last_row {
      None
    } else {
      // After swap_remove, the entity originally at last
      // index now occupies `row`.
      Some(self.entities[row_idx])
    }
  }

  // Used by World's test introspection helpers.
  #[allow(dead_code)]
  pub fn len(&self) -> usize {
    self.entities.len()
  }
}
