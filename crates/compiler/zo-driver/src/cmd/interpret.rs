use crate::cmd::Handle;

use clap::Parser;
use smol_str::SmolStr;

#[derive(Parser)]
pub(crate) struct Interpret {
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

impl Interpret {
  fn interpret(&self) -> Result<(), String> {
    todo!()
  }

  fn interpreting(&self) -> Result<(), String> {
    todo!()
  }
}

impl Handle for Interpret {
  #[inline]
  fn handle(&self) {
    match self.interpret() {
      Ok(_) => std::process::exit(0),
      Err(_) => std::process::exit(1),
    }
  }
}
