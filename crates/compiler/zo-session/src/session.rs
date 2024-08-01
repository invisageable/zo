use super::settings::Settings;

use zo_interner::interner::Interner;
use zo_reporter::reporter::Reporter;

use swisskit::profiler::Profiler;

use smol_str::SmolStr;

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

impl Session {
  /// Displays the profiler results.
  ///
  /// See also [`Profiler::profile`].
  #[inline]
  pub fn profile(&self) {
    if self.settings.has_profile() {
      self.profiler.profile()
    }
  }

  /// Measures the duration that a function takes.
  ///
  /// See also [`Profiler`].
  pub fn with_timing<T>(
    &mut self,
    name: impl Into<SmolStr>,
    f: impl FnOnce(&mut Self) -> T,
  ) -> T {
    if self.settings.has_profile() {
      self.profiler.start();

      let returns = f(self);

      self.profiler.end();
      self.profiler.add_profile(name);

      returns
    } else {
      f(self)
    }
  }
}

impl Default for Session {
  #[inline]
  fn default() -> Self {
    Self {
      settings: Settings::new(),
      interner: Interner::new(),
      reporter: Reporter::new(),
      profiler: Profiler::new(),
    }
  }
}
