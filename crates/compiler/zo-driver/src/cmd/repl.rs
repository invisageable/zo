use crate::cmd::Handle;
use crate::constants::{EXIT_CODE_ERROR, EXIT_CODE_SUCCESS};

use zo_error::Error;

#[derive(clap::Args, Debug)]
pub(crate) struct Repl {}

impl Repl {
  fn repl(&self) -> Result<(), Vec<Error>> {
    println!("read eval loop!");
    Ok(())
  }
}

impl Handle for Repl {
  fn handle(&self) {
    match self.repl() {
      Ok(_) => std::process::exit(EXIT_CODE_SUCCESS),
      Err(_) => std::process::exit(EXIT_CODE_ERROR),
    }
  }
}
