use criterion::{black_box, Criterion};

pub fn out_elastic(c: &mut Criterion) {
  let mut group = c.benchmark_group("out_elastic");

  group
    .confidence_level(0.99)
    .sample_size(1000)
    .sampling_mode(criterion::SamplingMode::Flat)
    .significance_level(0.1);

  let nums = (0..10_000)
    .map(|_num| rand::random::<f32>() * 1000.0)
    .collect::<Vec<_>>();

  group.bench_function("eazy", |b| {
    use eazy::oscillatory::elastic::OutElastic;
    use eazy::Curve;

    b.iter(|| {
      let _ = black_box(
        nums
          .iter()
          .map(|num| OutElastic.y(*num))
          .collect::<Vec<_>>(),
      );
    })
  });

  group.bench_function("bevy_tween", |b| {
    b.iter(|| {
      let _ = black_box(
        nums
          .iter()
          .map(|num| {
            bevy_tween::interpolation::EaseFunction::ElasticOut.sample(*num)
          })
          .collect::<Vec<_>>(),
      );
    })
  });

  group.bench_function("easings", |b| {
    b.iter(|| {
      let _ = black_box(
        nums
          .iter()
          .map(|num| easings::elastic_out(*num as f64))
          .collect::<Vec<_>>(),
      );
    })
  });

  group.bench_function("interpolation", |b| {
    use interpolation::Ease;

    b.iter(|| {
      let _ =
        black_box(nums.iter().map(|num| num.elastic_out()).collect::<Vec<_>>());
    })
  });

  group.bench_function("bevy_motiongfx", |b| {
    b.iter(|| {
      let _ = black_box(
        nums
          .iter()
          .map(|num| {
            bevy_motiongfx::motiongfx_core::ease::elastic::ease_out(*num)
          })
          .collect::<Vec<_>>(),
      );
    })
  });

  group.bench_function("simple_easing2", |b| {
    b.iter(|| {
      let _ = black_box(
        nums
          .iter()
          .map(|num| simple_easing2::elastic_out(*num))
          .collect::<Vec<_>>(),
      );
    })
  });

  group.finish();
}
