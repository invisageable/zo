//! Read-only queries (E3) and mutating queries (E4).
//!
//! 2-arg surface only for v1: `world.query2::<A, B>().iter()`
//! yields `(A, B)` per matching row. Other arities (1, 3+)
//! land when a real consumer needs them — YAGNI.
//!
//! Bound: `A: Copy + 'static`. Components are read out of
//! the column buffer by value (`ptr::read`). Storage is
//! aligned to each component's natural alignment, so the
//! reads don't need `read_unaligned` for soundness — but
//! we keep the by-value yield for E3's iterator API
//! (avoids LendingIterator gymnastics).
//!
//! Algorithm: pre-collect a `Vec<ArchetypeView>` of every
//! archetype containing both columns, then walk
//! `(archetype_idx, row)` flat. O(rows) total cost; no
//! per-row hash lookup.

use crate::world::World;

use std::any::TypeId;
use std::marker::PhantomData;

/// Per-archetype iteration state — raw pointers into the
/// two columns being read, plus the row count.
struct ArchetypeView<'w> {
  ptr_a: *const u8,
  ptr_b: *const u8,
  len: usize,
  _marker: PhantomData<&'w ()>,
}

pub struct Query2<'w, A, B> {
  world: &'w World,
  _phantom: PhantomData<(A, B)>,
}

impl<'w, A, B> Query2<'w, A, B>
where
  A: Copy + 'static,
  B: Copy + 'static,
{
  pub(crate) fn new(world: &'w World) -> Self {
    Self {
      world,
      _phantom: PhantomData,
    }
  }

  /// Build the iterator. Resolves `A`/`B`'s `ComponentId`
  /// and gathers every archetype that contains both —
  /// O(archetype_count), once per `iter()` call.
  pub fn iter(&self) -> Query2Iter<'w, A, B> {
    let id_a = match self.world.registry.id_of(TypeId::of::<A>()) {
      Some(id) => id,
      None => return Query2Iter::empty(),
    };
    let id_b = match self.world.registry.id_of(TypeId::of::<B>()) {
      Some(id) => id,
      None => return Query2Iter::empty(),
    };

    let mut views = Vec::new();
    for archetype in &self.world.archetypes {
      let col_a = match archetype.columns.get(&id_a) {
        Some(c) => c,
        None => continue,
      };
      let col_b = match archetype.columns.get(&id_b) {
        Some(c) => c,
        None => continue,
      };

      views.push(ArchetypeView {
        ptr_a: col_a.as_ptr(),
        ptr_b: col_b.as_ptr(),
        len: archetype.entities.len(),
        _marker: PhantomData,
      });
    }

    Query2Iter {
      views,
      arch_cursor: 0,
      row: 0,
      _phantom: PhantomData,
    }
  }

  /// Count matching rows without materialising values.
  /// Cheap — sums the `len` of each matching archetype.
  pub fn count(&self) -> usize {
    let id_a = match self.world.registry.id_of(TypeId::of::<A>()) {
      Some(id) => id,
      None => return 0,
    };
    let id_b = match self.world.registry.id_of(TypeId::of::<B>()) {
      Some(id) => id,
      None => return 0,
    };

    self
      .world
      .archetypes
      .iter()
      .filter(|a| {
        a.columns.contains_key(&id_a) && a.columns.contains_key(&id_b)
      })
      .map(|a| a.entities.len())
      .sum()
  }
}

pub struct Query2Iter<'w, A, B> {
  views: Vec<ArchetypeView<'w>>,
  arch_cursor: usize,
  row: usize,
  _phantom: PhantomData<(A, B)>,
}

impl<A, B> Query2Iter<'_, A, B> {
  fn empty() -> Self {
    Self {
      views: Vec::new(),
      arch_cursor: 0,
      row: 0,
      _phantom: PhantomData,
    }
  }
}

impl<A, B> Iterator for Query2Iter<'_, A, B>
where
  A: Copy + 'static,
  B: Copy + 'static,
{
  type Item = (A, B);

  fn next(&mut self) -> Option<(A, B)> {
    let size_a = std::mem::size_of::<A>();
    let size_b = std::mem::size_of::<B>();

    loop {
      if self.arch_cursor >= self.views.len() {
        return None;
      }

      let view = &self.views[self.arch_cursor];

      if self.row >= view.len {
        self.arch_cursor += 1;
        self.row = 0;
        continue;
      }

      // Storage is aligned to A/B's natural alignment, so
      // a plain `ptr::read` is sound. Copy bound means we
      // move bytes out without disturbing storage.
      let a = unsafe {
        std::ptr::read(view.ptr_a.add(self.row * size_a) as *const A)
      };
      let b = unsafe {
        std::ptr::read(view.ptr_b.add(self.row * size_b) as *const B)
      };

      self.row += 1;

      return Some((a, b));
    }
  }
}

/// Mutating 2-component query (E4).
///
/// Driven via a closure rather than a standard `Iterator`
/// to sidestep `LendingIterator`: each row is fed to the
/// closure as `(&mut A, &mut B)`, then the next row reuses
/// the same scratch slots — so two yielded refs never
/// alias, by construction.
pub struct Query2Mut<'w, A, B> {
  world: &'w mut World,
  _phantom: PhantomData<(A, B)>,
}

impl<'w, A, B> Query2Mut<'w, A, B>
where
  A: Copy + 'static,
  B: Copy + 'static,
{
  pub(crate) fn new(world: &'w mut World) -> Self {
    Self {
      world,
      _phantom: PhantomData,
    }
  }

  /// Run `f` once per matching row, with mutable access
  /// to a scratch copy of each component. The (possibly
  /// modified) values are flushed back to the column
  /// buffer when the closure returns.
  ///
  /// Hot loop walks raw pointers — both columns are
  /// fetched once per archetype via `get_disjoint_mut`,
  /// then `add(row * stride)` per row. No HashMap probe
  /// per element.
  pub fn for_each_mut<F>(&mut self, mut f: F)
  where
    F: FnMut(&mut A, &mut B),
  {
    let id_a = match self.world.registry.id_of(TypeId::of::<A>()) {
      Some(id) => id,
      None => return,
    };
    let id_b = match self.world.registry.id_of(TypeId::of::<B>()) {
      Some(id) => id,
      None => return,
    };

    // Same component twice would hand the same buffer
    // out via both refs — disallow rather than UB.
    debug_assert_ne!(id_a, id_b, "Query2Mut requires distinct component types");
    if id_a == id_b {
      return;
    }

    let size_a = std::mem::size_of::<A>();
    let size_b = std::mem::size_of::<B>();

    for archetype in &mut self.world.archetypes {
      let len = archetype.entities.len();
      if len == 0 {
        continue;
      }

      let [col_a, col_b] =
        match archetype.columns.get_disjoint_mut([&id_a, &id_b]) {
          [Some(a), Some(b)] => [a, b],
          _ => continue,
        };

      let ptr_a = col_a.as_mut_ptr();
      let ptr_b = col_b.as_mut_ptr();

      for row in 0..len {
        unsafe {
          let pa = ptr_a.add(row * size_a) as *mut A;
          let pb = ptr_b.add(row * size_b) as *mut B;

          // Storage is aligned to A/B's natural alignment,
          // so plain ptr::read/write is sound here.
          let mut a = std::ptr::read(pa);
          let mut b = std::ptr::read(pb);

          f(&mut a, &mut b);

          std::ptr::write(pa, a);
          std::ptr::write(pb, b);
        }
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use crate::World;

  #[derive(Clone, Copy, Debug, PartialEq)]
  struct Mesh {
    id: u32,
  }

  #[derive(Clone, Copy, Debug, PartialEq)]
  struct Transform {
    x: f32,
    y: f32,
  }

  #[derive(Clone, Copy, Debug, PartialEq)]
  struct Material {
    color: u32,
  }

  #[test]
  fn query_unregistered_yields_empty() {
    let w = World::new();
    let n = w.query2::<Mesh, Transform>().iter().count();
    assert_eq!(n, 0);
  }

  #[test]
  fn query_no_matching_archetype() {
    let mut w = World::new();
    let m = w.register::<Mesh>();
    // Spawn entities with only Mesh — none match (Mesh, Transform).
    for i in 0..10 {
      w.spawn().with(m, Mesh { id: i }).build();
    }
    let n = w.query2::<Mesh, Transform>().iter().count();
    assert_eq!(n, 0);
  }

  #[test]
  fn query_all_in_one_archetype() {
    let mut w = World::new();
    let m = w.register::<Mesh>();
    let t = w.register::<Transform>();

    for i in 0..5 {
      w.spawn()
        .with(m, Mesh { id: i })
        .with(
          t,
          Transform {
            x: i as f32,
            y: 0.0,
          },
        )
        .build();
    }

    let mut sum = 0u32;
    let mut count = 0;
    for (mesh, _xform) in w.query2::<Mesh, Transform>().iter() {
      sum += mesh.id;
      count += 1;
    }
    assert_eq!(count, 5);
    assert_eq!(sum, 1 + 2 + 3 + 4);
  }

  #[test]
  fn query_spans_multiple_archetypes() {
    // Entities with (M, T) and entities with (M, T, Material)
    // should both appear in a (M, T) query.
    let mut w = World::new();
    let m = w.register::<Mesh>();
    let t = w.register::<Transform>();
    let mat = w.register::<Material>();

    // 3 entities with (M, T).
    for i in 0..3 {
      w.spawn()
        .with(m, Mesh { id: i })
        .with(t, Transform { x: 0.0, y: 0.0 })
        .build();
    }
    // 2 entities with (M, T, Material).
    for i in 100..102 {
      w.spawn()
        .with(m, Mesh { id: i })
        .with(t, Transform { x: 0.0, y: 0.0 })
        .with(mat, Material { color: 0xFF })
        .build();
    }

    let q = w.query2::<Mesh, Transform>();
    assert_eq!(q.count(), 5);

    let ids: Vec<u32> = q.iter().map(|(m, _)| m.id).collect();
    assert_eq!(ids.len(), 5);
    // Order isn't specified; just check membership.
    assert!(ids.contains(&0));
    assert!(ids.contains(&100));
    assert!(ids.contains(&101));
  }

  #[test]
  fn query_skips_entities_missing_either_component() {
    let mut w = World::new();
    let m = w.register::<Mesh>();
    let t = w.register::<Transform>();

    // 3 with (M, T), 4 with only M.
    for i in 0..3 {
      w.spawn()
        .with(m, Mesh { id: i })
        .with(t, Transform { x: 0.0, y: 0.0 })
        .build();
    }
    for i in 100..104 {
      w.spawn().with(m, Mesh { id: i }).build();
    }

    let n = w.query2::<Mesh, Transform>().iter().count();
    assert_eq!(n, 3);
  }

  #[test]
  fn query_values_round_trip_correctly() {
    let mut w = World::new();
    let m = w.register::<Mesh>();
    let t = w.register::<Transform>();

    w.spawn()
      .with(m, Mesh { id: 42 })
      .with(t, Transform { x: 3.5, y: -7.0 })
      .build();

    let (mesh, xform) = w.query2::<Mesh, Transform>().iter().next().unwrap();
    assert_eq!(mesh.id, 42);
    assert_eq!(xform.x, 3.5);
    assert_eq!(xform.y, -7.0);
  }

  #[test]
  fn query_mut_unregistered_is_noop() {
    let mut w = World::new();
    let mut hit = 0;
    w.query2_mut::<Mesh, Transform>()
      .for_each_mut(|_, _| hit += 1);
    assert_eq!(hit, 0);
  }

  #[test]
  fn query_mut_no_matching_archetype() {
    let mut w = World::new();
    let m = w.register::<Mesh>();
    for i in 0..4 {
      w.spawn().with(m, Mesh { id: i }).build();
    }

    let mut hit = 0;
    w.query2_mut::<Mesh, Transform>()
      .for_each_mut(|_, _| hit += 1);
    assert_eq!(hit, 0);
  }

  #[test]
  fn query_mut_writes_back_within_archetype() {
    let mut w = World::new();
    let m = w.register::<Mesh>();
    let t = w.register::<Transform>();

    for i in 0..5 {
      w.spawn()
        .with(m, Mesh { id: i })
        .with(
          t,
          Transform {
            x: i as f32,
            y: 0.0,
          },
        )
        .build();
    }

    w.query2_mut::<Mesh, Transform>()
      .for_each_mut(|_mesh, xform| {
        xform.x += 100.0;
      });

    // Verify via the read-only query.
    let mut xs: Vec<f32> = w
      .query2::<Mesh, Transform>()
      .iter()
      .map(|(_, t)| t.x)
      .collect();
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert_eq!(xs, vec![100.0, 101.0, 102.0, 103.0, 104.0]);
  }

  #[test]
  fn query_mut_spans_multiple_archetypes() {
    let mut w = World::new();
    let m = w.register::<Mesh>();
    let t = w.register::<Transform>();
    let mat = w.register::<Material>();

    // 3 in (M, T).
    for i in 0..3 {
      w.spawn()
        .with(m, Mesh { id: i })
        .with(t, Transform { x: 1.0, y: 0.0 })
        .build();
    }
    // 2 in (M, T, Material).
    for i in 100..102 {
      w.spawn()
        .with(m, Mesh { id: i })
        .with(t, Transform { x: 1.0, y: 0.0 })
        .with(mat, Material { color: 0xFF })
        .build();
    }

    w.query2_mut::<Mesh, Transform>().for_each_mut(|_, xform| {
      xform.x *= 2.0;
    });

    let xs: Vec<f32> = w
      .query2::<Mesh, Transform>()
      .iter()
      .map(|(_, t)| t.x)
      .collect();
    assert_eq!(xs.len(), 5);
    for x in xs {
      assert_eq!(x, 2.0);
    }
  }

  #[test]
  fn query_mut_accumulates_across_frames() {
    // 60-frame integration check — each frame increments
    // every Transform.x by 1; final value must match.
    let mut w = World::new();
    let m = w.register::<Mesh>();
    let t = w.register::<Transform>();

    for i in 0..32 {
      w.spawn()
        .with(m, Mesh { id: i })
        .with(t, Transform { x: 0.0, y: 0.0 })
        .build();
    }

    for _ in 0..60 {
      w.query2_mut::<Mesh, Transform>().for_each_mut(|_, xform| {
        xform.x += 1.0;
      });
    }

    for (_, xform) in w.query2::<Mesh, Transform>().iter() {
      assert_eq!(xform.x, 60.0);
    }
  }
}
