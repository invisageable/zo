use crate::bench::BENCH_CONFIG;

use criterion::Criterion;

use std::hint::black_box;

pub fn out_circle(c: &mut Criterion) {
  let mut group = c.benchmark_group("out_circle");

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
        black_box(Easing::OutCircle.y(*num));
      }
    })
  });

  group.bench_function("easings", |b| {
    b.iter(|| {
      for num in nums.iter() {
        black_box(easings::circular_out(*num as f64));
      }
    })
  });

  group.bench_function("emath", |b| {
    use emath::easing;

    b.iter(|| {
      for num in nums.iter() {
        black_box(easing::circular_out(*num));
      }
    })
  });

  group.bench_function("interpolation", |b| {
    use interpolation::Ease;

    b.iter(|| {
      for num in nums.iter() {
        black_box(num.circular_out());
      }
    })
  });

  group.bench_function("nova-easing", |b| {
    use nova_easing::EasingArgument;

    b.iter(|| {
      for num in nums.iter() {
        black_box(num.ease_out_circ());
      }
    })
  });

  group.bench_function("simple_easing2", |b| {
    b.iter(|| {
      for num in nums.iter() {
        black_box(simple_easing2::circ_out(*num));
      }
    })
  });

  group.finish();
}
