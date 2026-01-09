use criterion::Criterion;

use std::hint::black_box;

pub fn out_quadratic(c: &mut Criterion) {
  let mut group = c.benchmark_group("out_quadratic");

  group
    .confidence_level(0.99)
    .sample_size(1000)
    .significance_level(0.05);

  group.bench_function("eazy", |b| {
    use eazy::Curve;
    use eazy::polynomial::quadratic::OutQuadratic;

    let nums = (0..10_000)
      .map(|_num| fastrand::f32() * 1000.0)
      .collect::<Vec<_>>();

    b.iter(|| {
      for num in nums.iter() {
        black_box(OutQuadratic.y(*num));
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
        black_box(easing::quadratic_out(*num));
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
        black_box(Easing::QuadraticOut.ease(*num));
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
        black_box(num.quadratic_out());
      }
    })
  });

  group.bench_function("nova-easing", |b| {
    use nova_easing::EasingArgument;

    let nums = (0..10_000)
      .map(|_num| fastrand::f32() * 1000.0)
      .collect::<Vec<_>>();

    b.iter(|| {
      for num in nums.iter() {
        black_box(num.ease_out_quad());
      }
    })
  });

  group.bench_function("simple_easing2", |b| {
    let nums = (0..10_000)
      .map(|_num| fastrand::f32() * 1000.0)
      .collect::<Vec<_>>();

    b.iter(|| {
      for num in nums.iter() {
        black_box(simple_easing2::quad_out(*num));
      }
    })
  });

  group.finish();
}
