use eazy::backtracking::back::InBack;
use eazy::{Curve, Easing, ease};

use criterion::Criterion;

use std::hint::black_box;

pub fn bench_internal_behavior(c: &mut Criterion) {
  let mut group = c.benchmark_group("curve_vs_ease");

  let nums = (0..10_000)
    .map(|_num| fastrand::f32() * 1000.0)
    .collect::<Vec<_>>();

  group.bench_function("Curve::y (direct)", |b| {
    b.iter(|| {
      nums.iter().for_each(|num| {
        black_box(InBack.y(black_box(*num)));
      });
    });
  });

  group.bench_function("ease(impl Curve)", |b| {
    b.iter(|| {
      nums.iter().for_each(|num| {
        black_box(ease(InBack, black_box(*num), 0.0, 1.0));
      });
    });
  });

  group.bench_function("ease(&dyn Curve)", |b| {
    let easing = Easing::InBack;

    b.iter(|| {
      let curve: &dyn Curve = &easing;

      nums.iter().for_each(|num| {
        black_box(ease(curve, black_box(*num), 0.0, 1.0));
      });
    });
  });

  group.finish();
}
