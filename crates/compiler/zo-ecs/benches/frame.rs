//! E6 acceptance bench: 10k entities, full frame < 1 ms.
//!
//! A "frame" = update pass (mut iter, advance Transform.x)
//! plus draw pass (read iter, stub `draw_mesh` per row).
//! 60 FPS budget is 16.7 ms / frame; the ECS portion must
//! stay under 10% (≈1 ms) so renderer/audio/input have room.

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

/// Renderer stub. The compiler can't elide it because
/// `black_box` consumes both refs — measures real
/// per-row dispatch cost.
#[inline(never)]
fn draw_mesh(mesh: &Mesh, xform: &Transform) {
  black_box(mesh);
  black_box(xform);
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

fn frame_10k(c: &mut Criterion) {
  let mut w = build_world();

  c.bench_function("frame_10k_update_then_draw", |b| {
    b.iter(|| {
      // Update pass: per-entity sim step.
      w.query2_mut::<Mesh, Transform>()
        .for_each_mut(|_mesh, xform| {
          xform.x += 1.0;
        });

      // Draw pass: hand each row to the renderer stub.
      for (mesh, xform) in w.query2::<Mesh, Transform>().iter() {
        draw_mesh(&mesh, &xform);
      }

      black_box(&mut w);
    });
  });
}

criterion_group!(benches, frame_10k);
criterion_main!(benches);
