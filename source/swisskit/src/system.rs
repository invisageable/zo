/// The representation of a system.
#[derive(Debug, Default)]
pub struct System {
  /// The system to fetch system information.
  info: sysinfo::System,
}

impl System {
  /// Creates a new system.
  #[inline]
  pub fn new() -> Self {
    let info = sysinfo::System::new();

    Self { info }
  }

  /// Refreshes all system and processes information.
  ///
  /// See also [`sysinfo::System::refresh_all`] for more information.
  #[inline]
  pub fn refresh_all(&mut self) {
    self.info.refresh_all();
  }

  /// Returns the RAM size in bytes.
  ///
  /// See also [`sysinfo::System::total_memory`] for more information.
  #[inline(always)]
  pub fn total(&self) -> u64 {
    self.info.total_memory()
  }

  /// Returns the amount of used RAM in bytes.
  ///
  /// See also [`sysinfo::System::used_memory`] for more information.
  #[inline(always)]
  pub fn used(&self) -> u64 {
    self.info.used_memory()
  }

  /// Returns the amount of free RAM in bytes.
  ///
  /// See also [`sysinfo::System::free_memory`] for more information.
  #[inline(always)]
  pub fn free(&self) -> u64 {
    self.info.free_memory()
  }

  /// Returns the list of the CPUs.
  ///
  /// See also [`sysinfo::System::free_memory`] for more information.
  #[inline]
  pub fn cpus(&self) -> Box<&[sysinfo::Cpu]> {
    Box::new(self.info.cpus())
  }

  /// Returns the number of CPUs.
  ///
  /// See also [`sysinfo::Cpu`] for more information.
  #[inline(always)]
  pub fn cpus_len(&self) -> usize {
    self.info.cpus().len()
  }
}
