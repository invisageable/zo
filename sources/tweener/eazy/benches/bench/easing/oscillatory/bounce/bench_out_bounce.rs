use crate::bench::BENCH_CONFIG;

use criterion::Criterion;

use std::hint::black_box;

pub fn out_bounce(c: &mut Criterion) {
  let mut group = c.benchmark_group("out_bounce");

  group
    .confidence_level(BENCH_CONFIG.confidence_level)
    .sample_size(BENCH_CONFIG.sample_size)
    .sampling_mode(BENCH_CONFIG.sampling_mode)
    .significance_level(BENCH_CONFIG.significance_level);

  let nums = (0..10_000)
    .map(|_num| fastrand::f32() * 1000.0)
    .collect::<Vec<_>>();

  group.bench_function("eazy", |b| {
    use eazy::{Curve, Easing};

    b.iter(|| {
      nums.iter().for_each(|num| {
        black_box(Easing::OutBounce.y(*num));
      });
    })
  });

  group.bench_function("easings", |b| {
    b.iter(|| {
      nums.iter().for_each(|num| {
        black_box(easings::bounce_out(*num as f64));
      });
    })
  });

  group.bench_function("emath", |b| {
    b.iter(|| {
      nums.iter().for_each(|num: &f32| {
        black_box(emath::easing::bounce_out(*num));
      });
    })
  });

  group.bench_function("interpolation", |b| {
    use interpolation::Ease;

    b.iter(|| {
      nums.iter().for_each(|num| {
        black_box(num.bounce_out());
      });
    })
  });

  group.bench_function("simple_easing2", |b| {
    b.iter(|| {
      nums.iter().for_each(|num| {
        black_box(simple_easing2::bounce_out(*num));
      })
    })
  });

  group.finish();
}
