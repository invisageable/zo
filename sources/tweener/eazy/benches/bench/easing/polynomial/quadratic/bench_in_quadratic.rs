use crate::bench::BENCH_CONFIG;

use criterion::Criterion;

use std::hint::black_box;

pub fn in_quadratic(c: &mut Criterion) {
  let mut group = c.benchmark_group("in_quadratic");

  group
    .confidence_level(BENCH_CONFIG.confidence_level)
    .sample_size(BENCH_CONFIG.sample_size)
    .significance_level(BENCH_CONFIG.significance_level);

  let nums = (0..10_000)
    .map(|_num| fastrand::f32() * 1000.0)
    .collect::<Vec<_>>();

  group.bench_function("eazy", |b| {
    use eazy::Curve;
    use eazy::polynomial::quadratic::InQuadratic;

    b.iter(|| {
      for num in nums.iter() {
        black_box(InQuadratic.y(*num));
      }
    })
  });

  group.bench_function("emath", |b| {
    use emath::easing;

    b.iter(|| {
      for num in nums.iter() {
        black_box(easing::quadratic_in(*num));
      }
    })
  });

  group.bench_function("glissade", |b| {
    use glissade::Easing;

    b.iter(|| {
      for num in nums.iter() {
        black_box(Easing::QuadraticIn.ease(*num));
      }
    })
  });

  group.bench_function("interpolation", |b| {
    use interpolation::Ease;

    b.iter(|| {
      for num in nums.iter() {
        black_box(num.quadratic_in());
      }
    })
  });

  group.bench_function("nova-easing", |b| {
    use nova_easing::EasingArgument;

    b.iter(|| {
      for num in nums.iter() {
        black_box(num.ease_in_quad());
      }
    })
  });

  group.bench_function("simple_easing2", |b| {
    b.iter(|| {
      for num in nums.iter() {
        black_box(simple_easing2::quad_in(*num));
      }
    })
  });

  group.finish();
}
