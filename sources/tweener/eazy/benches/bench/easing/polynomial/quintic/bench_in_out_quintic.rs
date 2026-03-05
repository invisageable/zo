use crate::bench::BENCH_CONFIG;

use criterion::Criterion;

use std::hint::black_box;

pub fn in_out_quintic(c: &mut Criterion) {
  let mut group = c.benchmark_group("in_out_quintic");

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
        black_box(Easing::InOutQuintic.y(*num));
      }
    })
  });

  group.bench_function("interpolation", |b| {
    use interpolation::Ease;

    b.iter(|| {
      for num in nums.iter() {
        black_box(num.quintic_in_out());
      }
    })
  });

  group.bench_function("simple_easing2", |b| {
    b.iter(|| {
      for num in nums.iter() {
        black_box(simple_easing2::quint_in_out(*num));
      }
    })
  });

  group.finish();
}
