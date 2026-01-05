use criterion::{Criterion, black_box};

pub fn in_out_elastic(c: &mut Criterion) {
  let mut group = c.benchmark_group("in_out_elastic");

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
    use eazy::oscillatory::elastic::InOutElastic;

    b.iter(|| {
      let _ = black_box(
        nums
          .iter()
          .map(|num| InOutElastic.y(*num))
          .collect::<Vec<_>>(),
      );
    })
  });

  group.bench_function("easings", |b| {
    b.iter(|| {
      let _ = black_box(
        nums
          .iter()
          .map(|num| easings::elastic_in_out(*num as f64))
          .collect::<Vec<_>>(),
      );
    })
  });

  group.bench_function("interpolation", |b| {
    use interpolation::Ease;

    b.iter(|| {
      let _ = black_box(
        nums
          .iter()
          .map(|num| num.elastic_in_out())
          .collect::<Vec<_>>(),
      );
    })
  });

  group.bench_function("bevy_motiongfx", |b| {
    b.iter(|| {
      let _ = black_box(
        nums
          .iter()
          .map(|num| {
            bevy_motiongfx::motiongfx_core::ease::elastic::ease_in_out(*num)
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
          .map(|num| simple_easing2::elastic_in_out(*num))
          .collect::<Vec<_>>(),
      );
    })
  });

  group.finish();
}
