use criterion::{Criterion, black_box};

pub fn out_cubic(c: &mut Criterion) {
  let mut group = c.benchmark_group("out_cubic");

  group
    .confidence_level(0.99)
    .sample_size(1000)
    .significance_level(0.05);

  group.bench_function("eazy", |b| {
    use eazy::Curve;
    use eazy::polynomial::cubic::OutCubic;

    let nums = (0..10_000)
      .map(|_num| rand::random::<f32>() * 1000.0)
      .collect::<Vec<_>>();

    b.iter(|| {
      for num in nums.iter() {
        black_box(OutCubic.y(*num));
      }
    })
  });

  group.bench_function("emath", |b| {
    use emath::easing;

    let nums = (0..10_000)
      .map(|_num| rand::random::<f32>() * 1000.0)
      .collect::<Vec<_>>();

    b.iter(|| {
      for num in nums.iter() {
        black_box(easing::cubic_out(*num));
      }
    })
  });

  group.bench_function("glissade", |b| {
    use glissade::Easing;

    let nums = (0..10_000)
      .map(|_num| rand::random::<f32>() * 1000.0)
      .collect::<Vec<_>>();

    b.iter(|| {
      for num in nums.iter() {
        black_box(Easing::CubicOut.ease(*num));
      }
    })
  });

  group.bench_function("interpolation", |b| {
    use interpolation::Ease;

    let nums = (0..10_000)
      .map(|_num| rand::random::<f32>() * 1000.0)
      .collect::<Vec<_>>();

    b.iter(|| {
      for num in nums.iter() {
        black_box(num.cubic_out());
      }
    })
  });

  group.bench_function("simple_easing2", |b| {
    let nums = (0..10_000)
      .map(|_num| rand::random::<f32>() * 1000.0)
      .collect::<Vec<_>>();

    b.iter(|| {
      for num in nums.iter() {
        black_box(simple_easing2::cubic_out(*num));
      }
    })
  });

  group.finish();
}
