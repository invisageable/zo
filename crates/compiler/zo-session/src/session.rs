use zo_interner::interner::Interner;
use zo_reporter::reporter::Reporter;

use swisskit::profiler::Profiler;

/// The representation of a compiler's session.
pub struct Session {
  /// The string interner.
  pub interner: Interner,
  /// The diagnostic reporter.
  pub reporter: Reporter,
  /// The time profiler.
  pub profiler: Profiler,
}

impl Session {
  /// Creates a new session.
  #[inline]
  pub fn new() -> Self {
    Self {
      interner: Interner::new(),
      reporter: Reporter::new(),
      profiler: Profiler::new(),
    }
  }
}
