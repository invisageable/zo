//! ...

#[derive(Debug, Default)]
pub struct System {
  pub(crate) info: sysinfo::System,
}

impl System {
  #[inline]
  pub fn new() -> Self {
    let info = sysinfo::System::new();

    Self { info }
  }

  #[inline]
  pub fn refresh_all(&mut self) {
    self.info.refresh_all();
  }

  #[inline]
  pub fn total(&self) -> u64 {
    self.info.total_memory()
  }

  #[inline]
  pub fn used(&self) -> u64 {
    self.info.used_memory()
  }

  #[inline]
  pub fn free(&self) -> u64 {
    self.info.free_memory()
  }

  #[inline]
  pub fn cpus(&self) -> usize {
    self.info.cpus().len()
  }

  #[inline]
  pub fn cpus_len(&self) -> usize {
    self.info.cpus().len()
  }
}
