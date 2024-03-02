//! ...

use super::settings::Settings;

use zo_core::interner::Interner;
use zo_core::profiler::Profiler;
use zo_core::reporter::Reporter;
use zo_core::system::System;

use smol_str::SmolStr;

#[derive(Debug)]
pub struct Session {
  pub settings: Settings,
  pub system: System,
  pub interner: Interner,
  pub reporter: Reporter,
  pub profiler: Profiler,
}

impl Session {
  pub fn with_timing<T>(
    &mut self,
    name: impl Into<SmolStr>,
    f: impl FnOnce(&mut Self) -> T,
  ) -> T {
    if self.settings.has_profiles() {
      self.profiler.add_profile(name);
      self.profiler.start();

      let returns = f(self);

      self.profiler.end();

      returns
    } else {
      f(self)
    }
  }
}

impl Default for Session {
  fn default() -> Self {
    Self {
      settings: Settings::new(),
      system: System::new(),
      interner: Interner::new(),
      reporter: Reporter::new(),
      profiler: Profiler::new(),
    }
  }
}
