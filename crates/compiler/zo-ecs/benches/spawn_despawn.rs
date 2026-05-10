//! E1 acceptance bench: 100k spawn + alternating despawn.
//! Target: full pass under 5 ms on a developer laptop.

use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use zo_ecs::World;

const N: usize = 100_000;

fn spawn_100k(c: &mut Criterion) {
  c.bench_function("spawn_100k", |b| {
    b.iter(|| {
      let mut w = World::with_capacity(N);

      for _ in 0..N {
        black_box(w.spawn().build());
      }

      assert_eq!(w.len(), N);
    });
  });
}

fn spawn_then_alternating_despawn(c: &mut Criterion) {
  c.bench_function("spawn_100k_alt_despawn", |b| {
    b.iter(|| {
      let mut w = World::with_capacity(N);
      let entities: Vec<_> = (0..N).map(|_| w.spawn().build()).collect();

      for (i, e) in entities.iter().enumerate() {
        if i % 2 == 0 {
          w.despawn(*e);
        }
      }

      assert_eq!(w.len(), N / 2);
    });
  });
}

fn churn_recycle(c: &mut Criterion) {
  // Steady-state: spawn N, despawn all, spawn N again →
  // second spawn should reuse the free list (no new allocs).
  c.bench_function("spawn_despawn_spawn", |b| {
    b.iter(|| {
      let mut w = World::with_capacity(N);
      let first: Vec<_> = (0..N).map(|_| w.spawn().build()).collect();

      for e in &first {
        w.despawn(*e);
      }

      for _ in 0..N {
        black_box(w.spawn().build());
      }

      assert_eq!(w.len(), N);
    });
  });
}

criterion_group!(
  benches,
  spawn_100k,
  spawn_then_alternating_despawn,
  churn_recycle
);
criterion_main!(benches);
