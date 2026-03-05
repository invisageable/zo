use crate::bench::BENCH_CONFIG;

use criterion::Criterion;

use std::hint::black_box;

pub fn out_cubic(c: &mut Criterion) {
  let mut group = c.benchmark_group("out_cubic");

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
        black_box(Easing::OutCubic.y(*num));
      }
    })
  });

  group.bench_function("emath", |b| {
    use emath::easing;

    b.iter(|| {
      for num in nums.iter() {
        black_box(easing::cubic_out(*num));
      }
    })
  });

  group.bench_function("glissade", |b| {
    use glissade::Easing;

    b.iter(|| {
      for num in nums.iter() {
        black_box(Easing::CubicOut.ease(*num));
      }
    })
  });

  group.bench_function("interpolation", |b| {
    use interpolation::Ease;

    b.iter(|| {
      for num in nums.iter() {
        black_box(num.cubic_out());
      }
    })
  });

  group.bench_function("simple_easing2", |b| {
    b.iter(|| {
      for num in nums.iter() {
        black_box(simple_easing2::cubic_out(*num));
      }
    })
  });

  group.finish();
}
