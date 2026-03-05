use crate::bench::BENCH_CONFIG;

use criterion::Criterion;

use std::hint::black_box;

pub fn in_out_expo2(c: &mut Criterion) {
  let mut group = c.benchmark_group("in_out_expo2");

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
      black_box(
        nums
          .iter()
          .map(|num| Easing::InOutExpo2.y(*num))
          .collect::<Vec<_>>(),
      );
    })
  });

  group.bench_function("emath", |b| {
    b.iter(|| {
      black_box(
        nums
          .iter()
          .map(|num| emath::easing::exponential_in_out(*num))
          .collect::<Vec<_>>(),
      );
    })
  });

  group.finish();
}
