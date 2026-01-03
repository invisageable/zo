// pub mod easing;
// pub mod interpolation;
// pub mod internal;

use std::sync::LazyLock;

pub(crate) static BENCH_CONFIG: LazyLock<BenchConfig> =
  LazyLock::new(|| BenchConfig {
    confidence_level: 0.99,
    sample_size: 500,
    significance_level: 0.05,
  });

/// The benchmark configuration.
pub(crate) struct BenchConfig {
  /// The size of bench samples.
  confidence_level: f64,
  /// The size of bench samples.
  sample_size: usize,
  /// The size of bench samples.
  significance_level: f64,
}
