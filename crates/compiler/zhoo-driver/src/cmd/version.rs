use crate::cmd::Handle;

use zo_core::{Result, EXIT_FAILURE, EXIT_SUCCESS};

use clap::Parser;
use smol_str::SmolStr;

#[derive(Parser)]
#[clap(about = "Show version")]
pub(crate) struct Version {
  #[clap(short, long, default_value = "false")]
  verbose: bool,
  #[clap(short, long)]
  input: SmolStr,
  #[clap(short, long, default_value = "wasm")]
  target: Option<SmolStr>,
}

impl Version {
  #[inline]
  fn version(&self) -> Result<()> {
    self.versioning()
  }

  #[inline]
  fn versioning(&self) -> Result<()> {
    println!("v{}", env!("CARGO_PKG_VERSION"));

    Ok(())
  }
}

impl Handle for Version {
  #[inline]
  fn handle(&self) {
    match self.version() {
      Ok(_) => std::process::exit(EXIT_SUCCESS),
      Err(_) => std::process::exit(EXIT_FAILURE),
    }
  }
}
