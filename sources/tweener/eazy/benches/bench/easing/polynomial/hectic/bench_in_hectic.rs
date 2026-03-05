use crate::bench::BENCH_CONFIG;

use criterion::Criterion;

use std::hint::black_box;

pub fn in_hectic(c: &mut Criterion) {
  let mut group = c.benchmark_group("in_hectic");

  group
    .confidence_level(BENCH_CONFIG.confidence_level)
    .sample_size(BENCH_CONFIG.sample_size)
    .significance_level(BENCH_CONFIG.significance_level);

  let nums = (0..10_000)
    .map(|_num| fastrand::f32() * 1000.0)
    .collect::<Vec<_>>();

  group.bench_function("eazy", |b| {
    use eazy::{Curve, Easing};

    b.iter(|| {
      for num in nums.iter() {
        black_box(Easing::InHectic.y(*num));
      }
    })
  });

  group.finish();
}
