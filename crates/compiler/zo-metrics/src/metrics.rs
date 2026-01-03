use crate::stats::Stats;

/// Represents a [`Metrics`] instance.
pub struct Metrics {
  /// The compiler statistics.
  stats: Stats,
}
impl Metrics {
  /// Creates a new [`Metrics`].
  pub fn new() -> Self {
    Self {}
  }
}
