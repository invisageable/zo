use super::backend::Backend;

use smol_str::SmolStr;

/// The representation of a settings session.
#[derive(Debug, Default)]
pub struct Settings {
  /// The pathname of the input source code.
  pub input: SmolStr,
  /// The backend to used for the code generation or the interpretation.
  pub backend: Backend,
  /// The profile flag to enable the profiler output.
  pub profile: std::sync::Arc<std::sync::atomic::AtomicBool>,
  /// The verbose flag to enable the displaying of each compiler phases.
  pub verbose: std::sync::Arc<std::sync::atomic::AtomicBool>,
  /// The interactive flag to enable the `repl` mode.
  pub interactive: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Settings {
  /// Creates a new settings.
  #[inline]
  pub fn new() -> Self {
    Self::default()
  }

  /// Checks if the verbose mode is enabled.
  #[inline]
  pub fn has_verbose(&self) -> bool {
    self.verbose.load(std::sync::atomic::Ordering::Relaxed)
  }

  /// Checks if the profiling mode is enabled.
  #[inline]
  pub fn has_profile(&self) -> bool {
    self.profile.load(std::sync::atomic::Ordering::Relaxed)
  }

  /// Checks if the interactive mode is enabled.
  #[inline]
  pub fn is_interactive(&self) -> bool {
    self.interactive.load(std::sync::atomic::Ordering::Relaxed)
  }
}
