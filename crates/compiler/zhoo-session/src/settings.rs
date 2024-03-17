//! ...

use super::backend::Backend;

use smol_str::SmolStr;

#[derive(Debug, Default)]
pub struct Settings {
  pub input: SmolStr,
  pub backend: Backend,
  pub profile: std::sync::Arc<std::sync::atomic::AtomicBool>,
  pub verbose: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Settings {
  #[inline]
  pub fn new() -> Self {
    Self::default()
  }

  #[inline]
  pub fn has_verbose(&self) -> bool {
    self.verbose.load(std::sync::atomic::Ordering::Relaxed)
  }

  #[inline]
  pub fn has_profile(&self) -> bool {
    self.profile.load(std::sync::atomic::Ordering::Relaxed)
  }
}
