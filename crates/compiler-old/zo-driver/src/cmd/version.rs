use crate::cmd::Execute;

use zo_reporter::Result;

use swisskit::global::{EXIT_FAILURE, EXIT_SUCCESS};

use clap::Parser;

/// The `version` command.
#[derive(Parser)]
pub(crate) struct Version;
impl Version {
  /// Executes the `version` command.
  #[inline(always)]
  fn version(&self) -> Result<()> {
    self.versioning()
  }

  /// Typewrites the `zo` version compiler.
  #[inline(always)]
  fn versioning(&self) -> Result<()> {
    println!("v{}", env!("CARGO_PKG_VERSION"));

    Ok(())
  }
}

impl Execute for Version {
  #[inline(always)]
  fn exec(&self) {
    match self.version() {
      Ok(_) => std::process::exit(EXIT_SUCCESS),
      Err(_) => std::process::exit(EXIT_FAILURE),
    }
  }
}
