//! Easing function benchmarks comparing eazy with other crates.
//!
//! Run with: `cargo bench -p eazy --bench easing`
//!
//! Competitors:
//! - bevy_tween
//! - easings
//! - emath
//! - interpolation
//! - lilt
//! - simple_easing2

use criterion::{Criterion, criterion_group, criterion_main};

use std::hint::black_box;

// ============================================================================
// Configuration
// ============================================================================

const SAMPLE_COUNT: usize = 10_000;

/// Generate test values once, reuse across all benchmarks.
fn test_values() -> Vec<f32> {
  (0..SAMPLE_COUNT)
    .map(|i| i as f32 / SAMPLE_COUNT as f32)
    .collect()
}

// ============================================================================
// Macro for benchmarking a single easing function across all crates
// ============================================================================

/// Benchmark an easing function across multiple implementations.
///
/// Usage: `bench_easing!(group, "quad_in", eazy: InQuadratic, emath: quadratic_in, ...)`
macro_rules! bench_easing {
  ($group:expr, $nums:expr, {
    eazy: $eazy_curve:expr,
    $(emath: $emath_fn:ident,)?
    $(easings: $easings_fn:ident,)?
    $(interpolation: $interp_method:ident,)?
    $(lilt: $lilt_variant:ident,)?
    $(simple_easing2: $se2_fn:ident,)?
  }) => {
    // eazy (ours)
    $group.bench_function("eazy", |b| {
      use eazy::Curve;
      b.iter(|| {
        for &t in $nums.iter() {
          black_box($eazy_curve.y(t));
        }
      })
    });

    // emath (egui's math library)
    $(
      $group.bench_function("emath", |b| {
        use emath::easing;
        b.iter(|| {
          for &t in $nums.iter() {
            black_box(easing::$emath_fn(t));
          }
        })
      });
    )?

    // easings crate
    $(
      $group.bench_function("easings", |b| {
        b.iter(|| {
          for &t in $nums.iter() {
            black_box(easings::$easings_fn(t as f64));
          }
        })
      });
    )?

    // interpolation crate
    $(
      $group.bench_function("interpolation", |b| {
        use interpolation::Ease;
        b.iter(|| {
          for &t in $nums.iter() {
            black_box(t.$interp_method());
          }
        })
      });
    )?

    // lilt crate
    $(
      $group.bench_function("lilt", |b| {
        use lilt::Easing;
        b.iter(|| {
          for &t in $nums.iter() {
            black_box(Easing::$lilt_variant.value(t));
          }
        })
      });
    )?

    // simple_easing2 crate
    $(
      $group.bench_function("simple_easing2", |b| {
        b.iter(|| {
          for &t in $nums.iter() {
            black_box(simple_easing2::$se2_fn(t));
          }
        })
      });
    )?
  };
}

// ============================================================================
// Polynomial Easing Benchmarks
// ============================================================================

fn bench_quadratic(c: &mut Criterion) {
  let nums = test_values();

  // InQuadratic
  {
    let mut group = c.benchmark_group("polynomial/quadratic_in");
    bench_easing!(group, nums, {
      eazy: eazy::polynomial::quadratic::InQuadratic,
      emath: quadratic_in,
      easings: quadratic_in,
      interpolation: quadratic_in,
      lilt: EaseInQuad,
      simple_easing2: quad_in,
    });
    group.finish();
  }

  // OutQuadratic
  {
    let mut group = c.benchmark_group("polynomial/quadratic_out");
    bench_easing!(group, nums, {
      eazy: eazy::polynomial::quadratic::OutQuadratic,
      emath: quadratic_out,
      easings: quadratic_out,
      interpolation: quadratic_out,
      lilt: EaseOutQuad,
      simple_easing2: quad_out,
    });
    group.finish();
  }

  // InOutQuadratic
  {
    let mut group = c.benchmark_group("polynomial/quadratic_in_out");
    bench_easing!(group, nums, {
      eazy: eazy::polynomial::quadratic::InOutQuadratic,
      emath: quadratic_in_out,
      easings: quadratic_in_out,
      interpolation: quadratic_in_out,
      lilt: EaseInOutQuad,
      simple_easing2: quad_in_out,
    });
    group.finish();
  }
}

fn bench_cubic(c: &mut Criterion) {
  let nums = test_values();

  {
    let mut group = c.benchmark_group("polynomial/cubic_in");
    bench_easing!(group, nums, {
      eazy: eazy::polynomial::cubic::InCubic,
      emath: cubic_in,
      easings: cubic_in,
      interpolation: cubic_in,
      lilt: EaseInCubic,
      simple_easing2: cubic_in,
    });
    group.finish();
  }

  {
    let mut group = c.benchmark_group("polynomial/cubic_out");
    bench_easing!(group, nums, {
      eazy: eazy::polynomial::cubic::OutCubic,
      emath: cubic_out,
      easings: cubic_out,
      interpolation: cubic_out,
      lilt: EaseOutCubic,
      simple_easing2: cubic_out,
    });
    group.finish();
  }

  {
    let mut group = c.benchmark_group("polynomial/cubic_in_out");
    bench_easing!(group, nums, {
      eazy: eazy::polynomial::cubic::InOutCubic,
      emath: cubic_in_out,
      easings: cubic_in_out,
      interpolation: cubic_in_out,
      lilt: EaseInOutCubic,
      simple_easing2: cubic_in_out,
    });
    group.finish();
  }
}

fn bench_quartic(c: &mut Criterion) {
  let nums = test_values();

  {
    let mut group = c.benchmark_group("polynomial/quartic_in");
    bench_easing!(group, nums, {
      eazy: eazy::polynomial::quartic::InQuartic,
      easings: quartic_in,
      interpolation: quartic_in,
      lilt: EaseInQuart,
      simple_easing2: quart_in,
    });
    group.finish();
  }

  {
    let mut group = c.benchmark_group("polynomial/quartic_out");
    bench_easing!(group, nums, {
      eazy: eazy::polynomial::quartic::OutQuartic,
      easings: quartic_out,
      interpolation: quartic_out,
      lilt: EaseOutQuart,
      simple_easing2: quart_out,
    });
    group.finish();
  }

  {
    let mut group = c.benchmark_group("polynomial/quartic_in_out");
    bench_easing!(group, nums, {
      eazy: eazy::polynomial::quartic::InOutQuartic,
      easings: quartic_in_out,
      interpolation: quartic_in_out,
      lilt: EaseInOutQuart,
      simple_easing2: quart_in_out,
    });
    group.finish();
  }
}

fn bench_quintic(c: &mut Criterion) {
  let nums = test_values();

  {
    let mut group = c.benchmark_group("polynomial/quintic_in");
    bench_easing!(group, nums, {
      eazy: eazy::polynomial::quintic::InQuintic,
      easings: quintic_in,
      interpolation: quintic_in,
      lilt: EaseInQuint,
      simple_easing2: quint_in,
    });
    group.finish();
  }

  {
    let mut group = c.benchmark_group("polynomial/quintic_out");
    bench_easing!(group, nums, {
      eazy: eazy::polynomial::quintic::OutQuintic,
      easings: quintic_out,
      interpolation: quintic_out,
      lilt: EaseOutQuint,
      simple_easing2: quint_out,
    });
    group.finish();
  }

  {
    let mut group = c.benchmark_group("polynomial/quintic_in_out");
    bench_easing!(group, nums, {
      eazy: eazy::polynomial::quintic::InOutQuintic,
      easings: quintic_in_out,
      interpolation: quintic_in_out,
      lilt: EaseInOutQuint,
      simple_easing2: quint_in_out,
    });
    group.finish();
  }
}

// ============================================================================
// Trigonometric Easing Benchmarks
// ============================================================================

fn bench_sine(c: &mut Criterion) {
  let nums = test_values();

  {
    let mut group = c.benchmark_group("trigonometric/sine_in");
    bench_easing!(group, nums, {
      eazy: eazy::trigonometric::sine::InSine,
      easings: sin_in,
      interpolation: sine_in,
      simple_easing2: sine_in,
    });
    group.finish();
  }

  {
    let mut group = c.benchmark_group("trigonometric/sine_out");
    bench_easing!(group, nums, {
      eazy: eazy::trigonometric::sine::OutSine,
      easings: sin_out,
      interpolation: sine_out,
      simple_easing2: sine_out,
    });
    group.finish();
  }

  {
    let mut group = c.benchmark_group("trigonometric/sine_in_out");
    bench_easing!(group, nums, {
      eazy: eazy::trigonometric::sine::InOutSine,
      easings: sin_in_out,
      interpolation: sine_in_out,
      simple_easing2: sine_in_out,
    });
    group.finish();
  }
}

fn bench_circle(c: &mut Criterion) {
  let nums = test_values();

  {
    let mut group = c.benchmark_group("trigonometric/circle_in");
    bench_easing!(group, nums, {
      eazy: eazy::trigonometric::circle::InCircle,
      easings: circular_in,
      interpolation: circular_in,
      lilt: EaseInCirc,
      simple_easing2: circ_in,
    });
    group.finish();
  }

  {
    let mut group = c.benchmark_group("trigonometric/circle_out");
    bench_easing!(group, nums, {
      eazy: eazy::trigonometric::circle::OutCircle,
      easings: circular_out,
      interpolation: circular_out,
      lilt: EaseOutCirc,
      simple_easing2: circ_out,
    });
    group.finish();
  }

  {
    let mut group = c.benchmark_group("trigonometric/circle_in_out");
    bench_easing!(group, nums, {
      eazy: eazy::trigonometric::circle::InOutCircle,
      easings: circular_in_out,
      interpolation: circular_in_out,
      lilt: EaseInOutCirc,
      simple_easing2: circ_in_out,
    });
    group.finish();
  }
}

// ============================================================================
// Oscillatory Easing Benchmarks
// ============================================================================

fn bench_elastic(c: &mut Criterion) {
  let nums = test_values();

  {
    let mut group = c.benchmark_group("oscillatory/elastic_in");
    bench_easing!(group, nums, {
      eazy: eazy::oscillatory::elastic::InElastic,
      easings: elastic_in,
      interpolation: elastic_in,
      lilt: EaseInElastic,
      simple_easing2: elastic_in,
    });
    group.finish();
  }

  {
    let mut group = c.benchmark_group("oscillatory/elastic_out");
    bench_easing!(group, nums, {
      eazy: eazy::oscillatory::elastic::OutElastic,
      easings: elastic_out,
      interpolation: elastic_out,
      lilt: EaseOutElastic,
      simple_easing2: elastic_out,
    });
    group.finish();
  }

  {
    let mut group = c.benchmark_group("oscillatory/elastic_in_out");
    bench_easing!(group, nums, {
      eazy: eazy::oscillatory::elastic::InOutElastic,
      easings: elastic_in_out,
      interpolation: elastic_in_out,
      lilt: EaseInOutElastic,
      simple_easing2: elastic_in_out,
    });
    group.finish();
  }
}

fn bench_bounce(c: &mut Criterion) {
  let nums = test_values();

  {
    let mut group = c.benchmark_group("oscillatory/bounce_in");
    bench_easing!(group, nums, {
      eazy: eazy::oscillatory::bounce::InBounce,
      easings: bounce_in,
      interpolation: bounce_in,
      lilt: EaseInBounce,
      simple_easing2: bounce_in,
    });
    group.finish();
  }

  {
    let mut group = c.benchmark_group("oscillatory/bounce_out");
    bench_easing!(group, nums, {
      eazy: eazy::oscillatory::bounce::OutBounce,
      easings: bounce_out,
      interpolation: bounce_out,
      lilt: EaseOutBounce,
      simple_easing2: bounce_out,
    });
    group.finish();
  }

  {
    let mut group = c.benchmark_group("oscillatory/bounce_in_out");
    bench_easing!(group, nums, {
      eazy: eazy::oscillatory::bounce::InOutBounce,
      easings: bounce_in_out,
      interpolation: bounce_in_out,
      lilt: EaseInOutBounce,
      simple_easing2: bounce_in_out,
    });
    group.finish();
  }
}

// ============================================================================
// Backtracking Easing Benchmarks
// ============================================================================

fn bench_back(c: &mut Criterion) {
  let nums = test_values();

  {
    let mut group = c.benchmark_group("backtracking/back_in");
    bench_easing!(group, nums, {
      eazy: eazy::backtracking::back::InBack,
      emath: back_in,
      easings: back_in,
      interpolation: back_in,
      lilt: EaseInBack,
      simple_easing2: back_in,
    });
    group.finish();
  }

  {
    let mut group = c.benchmark_group("backtracking/back_out");
    bench_easing!(group, nums, {
      eazy: eazy::backtracking::back::OutBack,
      emath: back_out,
      easings: back_out,
      interpolation: back_out,
      lilt: EaseOutBack,
      simple_easing2: back_out,
    });
    group.finish();
  }

  {
    let mut group = c.benchmark_group("backtracking/back_in_out");
    bench_easing!(group, nums, {
      eazy: eazy::backtracking::back::InOutBack,
      emath: back_in_out,
      easings: back_in_out,
      interpolation: back_in_out,
      lilt: EaseInOutBack,
      simple_easing2: back_in_out,
    });
    group.finish();
  }
}

// ============================================================================
// Exponential Easing Benchmarks
// ============================================================================

fn bench_expo(c: &mut Criterion) {
  let nums = test_values();

  {
    let mut group = c.benchmark_group("exponential/expo_in");
    bench_easing!(group, nums, {
      eazy: eazy::exponential::expo2::InExpo2,
      easings: exponential_in,
      interpolation: exponential_in,
      lilt: EaseInExpo,
      simple_easing2: expo_in,
    });
    group.finish();
  }

  {
    let mut group = c.benchmark_group("exponential/expo_out");
    bench_easing!(group, nums, {
      eazy: eazy::exponential::expo2::OutExpo2,
      easings: exponential_out,
      interpolation: exponential_out,
      lilt: EaseOutExpo,
      simple_easing2: expo_out,
    });
    group.finish();
  }

  {
    let mut group = c.benchmark_group("exponential/expo_in_out");
    bench_easing!(group, nums, {
      eazy: eazy::exponential::expo2::InOutExpo2,
      easings: exponential_in_out,
      interpolation: exponential_in_out,
      lilt: EaseInOutExpo,
      simple_easing2: expo_in_out,
    });
    group.finish();
  }
}

// ============================================================================
// Criterion Groups
// ============================================================================

criterion_group!(
  polynomial_benches,
  bench_quadratic,
  bench_cubic,
  bench_quartic,
  bench_quintic,
);

criterion_group!(
  trigonometric_benches,
  bench_sine,
  bench_circle,
);

criterion_group!(
  oscillatory_benches,
  bench_elastic,
  bench_bounce,
);

criterion_group!(
  other_benches,
  bench_back,
  bench_expo,
);

criterion_main!(
  polynomial_benches,
  trigonometric_benches,
  oscillatory_benches,
  other_benches,
);
