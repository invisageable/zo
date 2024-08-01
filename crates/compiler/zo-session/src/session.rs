use super::settings::Settings;

use zo_interner::interner::Interner;
use zo_reporter::reporter::Reporter;

use swisskit::profiler::Profiler;

/// The representation of a compiler's session.
pub struct Session {
  /// The settings of the session.
  pub settings: Settings,
  /// The string interner.
  pub interner: Interner,
  /// The diagnostic reporter.
  pub reporter: Reporter,
  /// The time profiler.
  pub profiler: Profiler,
}

impl Default for Session {
  fn default() -> Self {
    Self {
      settings: Settings::new(),
      interner: Interner::new(),
      reporter: Reporter::new(),
      profiler: Profiler::new(),
    }
  }
}
