use criterion::{Criterion, black_box};

pub fn in_bounce(c: &mut Criterion) {
  let mut group = c.benchmark_group("in_bounce");

  group
    .confidence_level(0.99)
    .sample_size(1000)
    .sampling_mode(criterion::SamplingMode::Flat)
    .significance_level(0.1);

  let nums = (0..10_000)
    .map(|_num| rand::random::<f32>() * 1000.0)
    .collect::<Vec<_>>();

  group.bench_function("eazy", |b| {
    use eazy::Curve;
    use eazy::oscillatory::bounce::InBounce;

    b.iter(|| {
      let _ =
        black_box(nums.iter().map(|num| InBounce.y(*num)).collect::<Vec<_>>());
    })
  });

  group.bench_function("easings", |b| {
    b.iter(|| {
      let _ = black_box(
        nums
          .iter()
          .map(|num| easings::bounce_in(*num as f64))
          .collect::<Vec<_>>(),
      );
    })
  });

  group.bench_function("emath", |b| {
    b.iter(|| {
      let _ = black_box(
        nums
          .iter()
          .map(|num: &f32| emath::easing::bounce_in(*num))
          .collect::<Vec<_>>(),
      );
    })
  });

  group.bench_function("interpolation", |b| {
    use interpolation::Ease;

    b.iter(|| {
      let _ =
        black_box(nums.iter().map(|num| num.bounce_in()).collect::<Vec<_>>());
    })
  });

  group.bench_function("simple_easing2", |b| {
    b.iter(|| {
      let _ = black_box(
        nums
          .iter()
          .map(|num| simple_easing2::bounce_in(*num))
          .collect::<Vec<_>>(),
      );
    })
  });

  group.finish();
}
