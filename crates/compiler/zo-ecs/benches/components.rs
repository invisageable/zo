//! E2 acceptance bench: 10k entities with two-component
//! spawn into a single archetype. Validates that:
//!   - register/spawn/with/build chain holds throughput
//!   - archetype canonicalization keeps everything in one
//!     packed storage block
//!   - the column-major append cost stays linear

use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use zo_ecs::World;

const N: usize = 10_000;

#[derive(Clone, Copy)]
#[repr(C)]
struct Mesh {
  id: u32,
}

#[derive(Clone, Copy)]
#[repr(C)]
struct Transform {
  x: f32,
  y: f32,
  z: f32,
}

#[derive(Clone, Copy)]
#[repr(C)]
struct Material {
  color: u32,
}

fn spawn_10k_two_components(c: &mut Criterion) {
  c.bench_function("spawn_10k_mesh_transform", |b| {
    b.iter(|| {
      let mut w = World::new();
      let mesh_id = w.register::<Mesh>();
      let xform_id = w.register::<Transform>();

      for i in 0..N {
        let e = w
          .spawn()
          .with(mesh_id, Mesh { id: i as u32 })
          .with(
            xform_id,
            Transform {
              x: i as f32,
              y: 0.0,
              z: 0.0,
            },
          )
          .build();
        black_box(e);
      }

      assert_eq!(w.len(), N);
      // Empty archetype + (Mesh, Transform).
      assert_eq!(w.archetype_count(), 2);
    });
  });
}

fn spawn_10k_three_archetypes(c: &mut Criterion) {
  // Round-robin between three different component sets;
  // proves canonicalization keeps each set in its own
  // archetype regardless of insert order.
  c.bench_function("spawn_10k_round_robin_3_archetypes", |b| {
    b.iter(|| {
      let mut w = World::new();
      let m = w.register::<Mesh>();
      let t = w.register::<Transform>();
      let mat = w.register::<Material>();

      for i in 0..N {
        match i % 3 {
          0 => {
            w.spawn()
              .with(m, Mesh { id: i as u32 })
              .with(
                t,
                Transform {
                  x: 0.0,
                  y: 0.0,
                  z: 0.0,
                },
              )
              .build();
          }
          1 => {
            // Same set, opposite insert order.
            w.spawn()
              .with(
                t,
                Transform {
                  x: 0.0,
                  y: 0.0,
                  z: 0.0,
                },
              )
              .with(m, Mesh { id: i as u32 })
              .build();
          }
          _ => {
            w.spawn()
              .with(m, Mesh { id: i as u32 })
              .with(
                t,
                Transform {
                  x: 0.0,
                  y: 0.0,
                  z: 0.0,
                },
              )
              .with(mat, Material { color: i as u32 })
              .build();
          }
        }
      }

      // Empty + (Mesh, Transform) + (Mesh, Transform, Material).
      assert_eq!(w.archetype_count(), 3);
      assert_eq!(w.len(), N);
    });
  });
}

criterion_group!(
  benches,
  spawn_10k_two_components,
  spawn_10k_three_archetypes
);
criterion_main!(benches);
