//! E3 acceptance bench: 10k (Mesh, Transform) entities,
//! iterate per "frame", target < 100 µs per pass.

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

fn build_world() -> World {
  let mut w = World::new();
  let m = w.register::<Mesh>();
  let t = w.register::<Transform>();

  for i in 0..N {
    w.spawn()
      .with(m, Mesh { id: i as u32 })
      .with(
        t,
        Transform {
          x: i as f32,
          y: 0.0,
          z: 0.0,
        },
      )
      .build();
  }

  w
}

fn iter_10k_one_archetype(c: &mut Criterion) {
  let w = build_world();

  c.bench_function("iter_10k_mesh_transform", |b| {
    b.iter(|| {
      let mut sum: u64 = 0;
      for (mesh, xform) in w.query2::<Mesh, Transform>().iter() {
        sum += mesh.id as u64;
        sum += xform.x as u64;
      }
      black_box(sum);
    });
  });
}

fn count_10k(c: &mut Criterion) {
  let w = build_world();

  c.bench_function("count_10k_mesh_transform", |b| {
    b.iter(|| {
      let n = w.query2::<Mesh, Transform>().count();
      assert_eq!(n, N);
      black_box(n);
    });
  });
}

fn iter_mut_10k_transform(c: &mut Criterion) {
  let mut w = build_world();

  c.bench_function("iter_mut_10k_transform_x_inc", |b| {
    b.iter(|| {
      w.query2_mut::<Mesh, Transform>().for_each_mut(|_, xform| {
        xform.x += 1.0;
      });
      black_box(&mut w);
    });
  });
}

criterion_group!(
  benches,
  iter_10k_one_archetype,
  count_10k,
  iter_mut_10k_transform
);
criterion_main!(benches);
