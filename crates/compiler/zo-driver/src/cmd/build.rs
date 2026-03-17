use crate::cmd::Handle;
use crate::constants::EXIT_CODE_SUCCESS;
use crate::{args, constants::EXIT_CODE_ERROR};

use zo_compiler::{Compiler, Stage};
use zo_error::Error;

use std::path::PathBuf;

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

    // Build search paths for module resolution.
    let mut search_paths = Vec::new();

    // Standard library path: check ZO_STD_PATH env var,
    // then fall back to relative path from the binary.
    if let Ok(std_path) = std::env::var("ZO_STD_PATH") {
      search_paths.push(PathBuf::from(std_path));
    } else if let Ok(exe) = std::env::current_exe() {
      // Assume std lives at ../lib/std relative to binary.
      if let Some(parent) = exe.parent() {
        let std_path = parent.join("../lib/std");

        if std_path.is_dir() {
          search_paths.push(std_path);
        }
      }
    }

    // Project source directory (relative to first input file).
    if let Some((first_file, _)) = source_files.first()
      && let Some(parent) = first_file.parent()
    {
      search_paths.push(parent.to_path_buf());
    }

    let mut compiler = Compiler::with_search_paths(search_paths);

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
