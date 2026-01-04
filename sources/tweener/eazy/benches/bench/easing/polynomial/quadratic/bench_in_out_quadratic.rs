use criterion::Criterion;

use std::hint::black_box;

pub fn in_out_quadratic(c: &mut Criterion) {
  let mut group = c.benchmark_group("in_out_quadratic");

  group
    .confidence_level(0.99)
    .sample_size(1000)
    .significance_level(0.05);

  group.bench_function("eazy", |b| {
    use eazy::Curve;
    use eazy::polynomial::quadratic::InOutQuadratic;

    let nums = (0..10_000)
      .map(|_num| fastrand::f32() * 1000.0)
      .collect::<Vec<_>>();

    b.iter(|| {
      for num in nums.iter() {
        black_box(InOutQuadratic.y(*num));
      }
    })
  });

  group.bench_function("bevy_tween", |b| {
    use bevy_tween::interpolation::EaseKind;

    let nums = (0..10_000)
      .map(|_num| fastrand::f32() * 1000.0)
      .collect::<Vec<_>>();

    b.iter(|| {
      for num in nums.iter() {
        black_box(EaseKind::QuadraticInOut.sample(*num));
      }
    })
  });

  group.bench_function("emath", |b| {
    use emath::easing;

    let nums = (0..10_000)
      .map(|_num| fastrand::f32() * 1000.0)
      .collect::<Vec<_>>();

    b.iter(|| {
      for num in nums.iter() {
        black_box(easing::quadratic_in_out(*num));
      }
    })
  });

  group.bench_function("glissade", |b| {
    use glissade::Easing;

    let nums = (0..10_000)
      .map(|_num| fastrand::f32() * 1000.0)
      .collect::<Vec<_>>();

    b.iter(|| {
      for num in nums.iter() {
        black_box(Easing::QuadraticInOut.ease(*num));
      }
    })
  });

  group.bench_function("interpolation", |b| {
    use interpolation::Ease;

    let nums = (0..10_000)
      .map(|_num| fastrand::f32() * 1000.0)
      .collect::<Vec<_>>();

    b.iter(|| {
      for num in nums.iter() {
        black_box(num.quadratic_in_out());
      }
    })
  });

  group.bench_function("simple_easing2", |b| {
    let nums = (0..10_000)
      .map(|_num| fastrand::f32() * 1000.0)
      .collect::<Vec<_>>();

    b.iter(|| {
      for num in nums.iter() {
        black_box(simple_easing2::quad_in_out(*num));
      }
    })
  });

  group.finish();
}
