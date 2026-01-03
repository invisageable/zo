use criterion::{Criterion, black_box};

use crate::bench::BENCH_CONFIG;

pub fn out_back(c: &mut Criterion) {
  let mut group = c.benchmark_group("out_back");

  group
    .confidence_level(BENCH_CONFIG.confidence_level)
    .sample_size(BENCH_CONFIG.sample_size)
    .significance_level(BENCH_CONFIG.significance_level);

  group.bench_function("eazy", |b| {
    use eazy::Curve;
    use eazy::backtracking::back::OutBack;

    let nums = (0..10_000)
      .map(|_num| rand::random::<f32>() * 1000.0)
      .collect::<Vec<_>>();

    b.iter(|| {
      for num in nums.iter() {
        black_box(OutBack.y(*num % 1.0));
      }
    })
  });

  group.bench_function("bevy_tween", |b| {
    use bevy_tween::interpolation::EaseKind;

    let nums = (0..10_000)
      .map(|_num| rand::random::<f32>() * 1000.0)
      .collect::<Vec<_>>();

    b.iter(|| {
      for num in nums.iter() {
        black_box(EaseKind::BackOut.sample(*num % 1.0));
      }
    })
  });

  group.bench_function("easings", |b| {
    let nums = (0..10_000)
      .map(|_num| rand::random::<f32>() * 1000.0)
      .collect::<Vec<_>>();

    b.iter(|| {
      for num in nums.iter() {
        black_box(easings::back_out((*num % 1.0) as f64));
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
        black_box(easing::back_out(*num % 1.0));
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
        black_box(num.back_out());
      }
    })
  });

  group.bench_function("lilt", |b| {
    use lilt::Easing;

    let nums = (0..10_000)
      .map(|_num| rand::random::<f32>() * 1000.0)
      .collect::<Vec<_>>();

    b.iter(|| {
      for num in nums.iter() {
        black_box(Easing::EaseOutBack.value(*num % 1.0));
      }
    })
  });

  group.bench_function("motiongfx", |b| {
    use motiongfx::prelude::ease;

    let nums = (0..10_000)
      .map(|_num| rand::random::<f32>() * 1000.0)
      .collect::<Vec<_>>();

    b.iter(|| {
      for num in nums.iter() {
        black_box(ease::back::ease_out(*num % 1.0));
      }
    })
  });

  group.bench_function("simple_easing2", |b| {
    let nums = (0..10_000)
      .map(|_num| rand::random::<f32>() * 1000.0)
      .collect::<Vec<_>>();

    b.iter(|| {
      for num in nums.iter() {
        black_box(simple_easing2::back_out(*num % 1.0));
      }
    })
  });

  group.finish();
}
