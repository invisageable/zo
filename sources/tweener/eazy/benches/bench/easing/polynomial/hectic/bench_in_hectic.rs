use criterion::{Criterion, black_box};

pub fn in_hectic(c: &mut Criterion) {
  let mut group = c.benchmark_group("in_hectic");

  group
    .confidence_level(0.99)
    .sample_size(1000)
    .significance_level(0.05);

  group.bench_function("eazy", |b| {
    use eazy::Curve;
    use eazy::polynomial::hectic::InHectic;

    let nums = (0..10_000)
      .map(|_num| rand::random::<f32>() * 1000.0)
      .collect::<Vec<_>>();

    b.iter(|| {
      for num in nums.iter() {
        black_box(InHectic.y(*num));
      }
    })
  });

  group.finish();
}
