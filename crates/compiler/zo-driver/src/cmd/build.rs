use crate::cmd::Handle;
use crate::constants::EXIT_CODE_SUCCESS;
use crate::{args, constants::EXIT_CODE_ERROR};

use zo_compiler::{Compiler, Stage};
use zo_error::Error;

#[derive(clap::Args, Debug)]
pub(crate) struct Build {
  #[command(flatten)]
  pub(crate) args: args::Args,
}

impl Build {
  fn build(&self) -> Result<(), Error> {
    let mut source_files = Vec::new();

    for input_path in &self.args.files {
      if !input_path.exists() {
        eprintln!("Error: File not found: {}", input_path.display());
        std::process::exit(EXIT_CODE_ERROR);
      }

      let content = match std::fs::read_to_string(input_path) {
        Ok(c) => c,
        Err(error) => {
          eprintln!("Error reading file {}: {error}", input_path.display());
          std::process::exit(EXIT_CODE_ERROR);
        }
      };

      source_files.push((input_path, content));
    }

    let mut compiler = Compiler::new();

    let stages = self
      .args
      .emit
      .iter()
      .map(|stage| match stage {
        args::Stage::Tokens => Stage::Tokens,
        args::Stage::Tree => Stage::Tree,
        args::Stage::Sir => Stage::Sir,
        args::Stage::Asm => Stage::Asm,
        args::Stage::All => Stage::All,
      })
      .collect::<Vec<_>>();

    compiler.compile(
      &source_files,
      self.args.target.into(),
      &stages,
      &self.args.output,
    )
  }
}

impl Handle for Build {
  fn handle(&self) {
    match self.build() {
      Ok(_) => std::process::exit(EXIT_CODE_SUCCESS),
      Err(_) => std::process::exit(EXIT_CODE_ERROR),
    }
  }
}
