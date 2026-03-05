use crate::bench::BENCH_CONFIG;

use criterion::Criterion;

use std::hint::black_box;

pub fn in_out_bounce(c: &mut Criterion) {
  let mut group = c.benchmark_group("in_out_bounce");

  group
    .confidence_level(BENCH_CONFIG.confidence_level)
    .sample_size(BENCH_CONFIG.sample_size)
    .sampling_mode(BENCH_CONFIG.sampling_mode)
    .significance_level(BENCH_CONFIG.significance_level);

  let nums = (0..10_000)
    .map(|_num| fastrand::f32() * 1000.0)
    .collect::<Vec<_>>();

  group.bench_function("eazy", |b| {
    use eazy::{Curve, Easing};

    b.iter(|| {
      nums.iter().for_each(|num| {
        black_box(Easing::InOutBounce.y(*num));
      })
    })
  });

  group.bench_function("easings", |b| {
    b.iter(|| {
      nums.iter().for_each(|num| {
        black_box(easings::bounce_in_out(*num as f64));
      });
    })
  });

  group.bench_function("emath", |b| {
    b.iter(|| {
      nums.iter().for_each(|num: &f32| {
        black_box(emath::easing::bounce_in_out(*num));
      });
    })
  });

  // group.bench_function("gpui", |b| {
  //   b.iter(|| {
  //     black_box(
  //       nums
  //         .iter()
  //         .map(|num: &f32| gpui::bounce(gpui::ease_in_out)(*num))
  //         .collect::<Vec<_>>(),
  //     );
  //   })
  // });

  group.bench_function("interpolation", |b| {
    use interpolation::Ease;

    b.iter(|| {
      nums.iter().for_each(|num| {
        black_box(num.bounce_in_out());
      });
    })
  });

  group.bench_function("simple_easing2", |b| {
    b.iter(|| {
      nums.iter().for_each(|num| {
        black_box(simple_easing2::bounce_in_out(*num));
      });
    })
  });

  group.finish();
}
