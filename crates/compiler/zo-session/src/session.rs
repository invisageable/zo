use super::settings::Settings;

use zo_interner::interner::Interner;
use zo_reporter::reporter::Reporter;

use swisskit::profiler::Profiler;

use lazy_static::lazy_static;
use smol_str::SmolStr;

/// The representation of a compiler's session.
///
/// The session is used in each compiler phases.
#[derive(Clone, Debug)]
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

  /// Sets the settings of the session.
  #[inline(always)]
  pub fn with_settings(&mut self, settings: Settings) {
    self.settings = settings;
  }

  /// Measures the duration that a function takes — see also [`Profiler`].
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
  #[inline(always)]
  fn default() -> Self {
    Self {
      settings: Settings::new(),
      interner: Interner::new(),
      reporter: Reporter::new(),
      profiler: Profiler::new(),
    }
  }
}

lazy_static! {
  /// The session wrap into an [`Arc<Mutex>`] for multithreading.
  pub static ref SESSION: std::sync::Arc<std::sync::Mutex<Session>> =
    std::sync::Arc::new(std::sync::Mutex::new(Session::default()));
}
