use crate::cmd::Handle;

use clap::Parser;
use smol_str::SmolStr;

#[derive(Parser)]
pub(crate) struct Build {
  #[clap(short, long, default_value = "false")]
  verbose: bool,
  #[clap(short, long)]
  input: SmolStr,
  #[clap(short, long, default_value = "wasm")]
  backend: SmolStr,
  #[clap(short, long, default_value = "false")]
  release: bool,
  #[clap(short, long, default_value = "false")]
  profile: bool,
}

impl Build {
  fn compile(&self) -> Result<(), String> {
    todo!()
  }

  fn compiling(&self) -> Result<(), String> {
    todo!()
  }
}

impl Handle for Build {
  #[inline]
  fn handle(&self) {
    match self.compile() {
      Ok(_) => std::process::exit(0),
      Err(_) => std::process::exit(1),
    }
  }
}
