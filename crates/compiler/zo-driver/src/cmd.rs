mod build;
mod repl;
mod run;

use clap::Subcommand;

use std::path::{Path, PathBuf};

pub(crate) trait Handle {
  /// Handles the execution of a command.
  fn handle(&self);
}

/// Build search paths for module resolution: ZO_STD_PATH
/// env, installed/dev std lib, and the input file's
/// parent directory.
pub(crate) fn search_paths(input: &Path) -> Vec<PathBuf> {
  let mut paths = Vec::new();

  if let Ok(std_path) = std::env::var("ZO_STD_PATH") {
    paths.push(PathBuf::from(std_path));
  } else if let Ok(exe) = std::env::current_exe()
    && let Some(parent) = exe.parent()
  {
    let installed = parent.join("../lib/std");
    let dev = parent.join("../../crates/compiler-lib/std");

    if installed.is_dir() {
      paths.push(installed);
    } else if dev.is_dir() {
      paths.push(dev);
    }
  }

  if let Some(parent) = input.parent() {
    paths.push(parent.to_path_buf());
  }

  paths
}

/// Reads source from a file, exiting on error.
pub(crate) fn read_source(path: &Path) -> String {
  if !path.exists() {
    eprintln!("Error: File not found: {}", path.display());

    std::process::exit(super::constants::EXIT_CODE_ERROR);
  }

  match std::fs::read_to_string(path) {
    Ok(c) => c,
    Err(error) => {
      eprintln!("Error reading file {}: {error}", path.display());

      std::process::exit(super::constants::EXIT_CODE_ERROR);
    }
  }
}

/// Represents a [`Cmd`] enumeration.
#[derive(Debug, Subcommand)]
pub(crate) enum Cmd {
  /// build a program.
  Build(build::Build),
  /// read eval print and loop a program (not implemented yet).
  Repl(repl::Repl),
  /// run a program.
  Run(run::Run),
}
