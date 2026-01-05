use criterion::{Criterion, black_box};

pub fn out_expo2(c: &mut Criterion) {
  let mut group = c.benchmark_group("out_expo2");

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
    use eazy::exponential::expo2::OutExpo2;

    b.iter(|| {
      let _ =
        black_box(nums.iter().map(|num| OutExpo2.y(*num)).collect::<Vec<_>>());
    })
  });

  group.bench_function("emath", |b| {
    b.iter(|| {
      let _ = black_box(
        nums
          .iter()
          .map(|num| emath::easing::exponential_out(*num))
          .collect::<Vec<_>>(),
      );
    })
  });

  group.bench_function("bevy_motiongfx", |b| {
    b.iter(|| {
      let _ = black_box(
        nums
          .iter()
          .map(|num| bevy_motiongfx::motiongfx_core::ease::expo::ease_out(*num))
          .collect::<Vec<_>>(),
      );
    })
  });

  group.finish();
}
