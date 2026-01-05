use criterion::{Criterion, black_box};

pub fn out_sine(c: &mut Criterion) {
  let mut group = c.benchmark_group("out_sine");

  group
    .confidence_level(0.99)
    .sample_size(1000)
    .significance_level(0.05);

  group.bench_function("eazy", |b| {
    use eazy::Curve;
    use eazy::trigonometric::sine::OutSine;

    let nums = (0..10_000)
      .map(|_num| rand::random::<f32>() * 1000.0)
      .collect::<Vec<_>>();

    b.iter(|| {
      for num in nums.iter() {
        black_box(OutSine.y(*num));
      }
    })
  });

  // note(ivs): should be isolate until changing the signature of my easing functions.
  group.bench_function("easings", |b| {
    let nums = (0..10_000)
      .map(|_num| rand::random::<f32>() * 1000.0)
      .collect::<Vec<_>>();

    b.iter(|| {
      for num in nums.iter() {
        black_box(easings::sin_out(*num as f64));
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
        black_box(easing::sin_out(*num));
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
        black_box(num.sine_out());
      }
    })
  });

  group.bench_function("simple_easing2", |b| {
    let nums = (0..10_000)
      .map(|_num| rand::random::<f32>() * 1000.0)
      .collect::<Vec<_>>();

    b.iter(|| {
      for num in nums.iter() {
        black_box(simple_easing2::sine_out(*num));
      }
    })
  });

  group.finish();
}
