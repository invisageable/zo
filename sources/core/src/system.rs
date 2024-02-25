#[derive(Debug, Default)]
pub struct System {
  pub(crate) info: sysinfo::System,
}

impl System {
  pub fn new() -> Self {
    let info = sysinfo::System::new();

    Self { info }
  }

  pub fn refresh_all(&mut self) {
    self.info.refresh_all();
  }

  pub fn total(&self) -> u64 {
    self.info.total_memory()
  }

  pub fn used(&self) -> u64 {
    self.info.used_memory()
  }

  pub fn free(&self) -> u64 {
    self.info.free_memory()
  }

  pub fn cpus(&self) -> usize {
    self.info.cpus().len()
  }

  pub fn cpus_len(&self) -> usize {
    self.info.cpus().len()
  }
}
