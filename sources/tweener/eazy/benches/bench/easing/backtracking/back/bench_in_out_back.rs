use crate::bench::BENCH_CONFIG;

use criterion::Criterion;

use std::hint::black_box;

pub fn in_out_back(c: &mut Criterion) {
  let mut group = c.benchmark_group("in_out_back");

  group
    .confidence_level(BENCH_CONFIG.confidence_level)
    .sample_size(BENCH_CONFIG.sample_size)
    .significance_level(BENCH_CONFIG.significance_level);

  let nums = (0..10_000)
    .map(|_num| fastrand::f32() * 1000.0)
    .collect::<Vec<_>>();

  group.bench_function("eazy", |b| {
    use eazy::Curve;
    use eazy::backtracking::back::InOutBack;

    b.iter(|| {
      for num in nums.iter() {
        black_box(InOutBack.y(*num % 1.0));
      }
    })
  });

  // group.bench_function("bevy_tween", |b| {
  //   use bevy_tween::interpolation::EaseKind;

  //   b.iter(|| {
  //     for num in nums.iter() {
  //       black_box(EaseKind::BackInOut.sample(*num % 1.0));
  //     }
  //   })
  // });

  group.bench_function("easings", |b| {
    b.iter(|| {
      for num in nums.iter() {
        black_box(easings::back_in_out((*num % 1.0) as f64));
      }
    })
  });

  group.bench_function("emath", |b| {
    use emath::easing;

    b.iter(|| {
      for num in nums.iter() {
        black_box(easing::back_in_out(*num % 1.0));
      }
    })
  });

  group.bench_function("interpolation", |b| {
    use interpolation::Ease;

    b.iter(|| {
      for num in nums.iter() {
        black_box(num.back_in_out());
      }
    })
  });

  group.bench_function("lilt", |b| {
    use lilt::Easing;

    b.iter(|| {
      for num in nums.iter() {
        black_box(Easing::EaseInOutBack.value(*num % 1.0));
      }
    })
  });

  group.bench_function("nova-easing", |b| {
    use nova_easing::EasingArgument;

    b.iter(|| {
      for num in nums.iter() {
        black_box((*num % 1.0).ease_in_out_back());
      }
    })
  });

  group.bench_function("simple_easing2", |b| {
    b.iter(|| {
      for num in nums.iter() {
        black_box(simple_easing2::back_in_out(*num % 1.0));
      }
    })
  });

  group.finish();
}
