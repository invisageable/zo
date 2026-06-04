mod build;
mod repl;
mod run;
mod test;

use crate::args;
use crate::constants::{EXIT_CODE_ERROR, EXIT_CODE_SUCCESS};
use crate::watch;

use zo_error::Error;

use clap::Subcommand;

use std::path::{Path, PathBuf};
use std::{env, process};

pub(crate) trait Handle {
  /// Handles the execution of a command.
  fn handle(&self);
}

/// Drive a `Result<(), Error>`-returning one-shot handler
/// either as a single run or as a `--watch` loop, then
/// exit. `args.files[0]` is safe to index — clap enforces
/// `required = true` on the field at parse time.
pub(crate) fn handle_with_watch(
  args: &args::Args,
  mut once: impl FnMut() -> Result<(), Error>,
) -> ! {
  if args.watch {
    if let Err(error) = watch::watch_loop(&args.files[0], || {
      // Errors are surfaced through the diagnostics path;
      // the loop deliberately never bails on them.
      let _ = once();
    }) {
      eprintln!("watch: {error}");

      process::exit(EXIT_CODE_ERROR);
    }

    process::exit(EXIT_CODE_SUCCESS);
  }

  match once() {
    Ok(_) => process::exit(EXIT_CODE_SUCCESS),
    Err(_) => process::exit(EXIT_CODE_ERROR),
  }
}

/// Build search paths for module resolution: ZO_CORE_PATH
/// env, installed/dev system-pack layout (`core` + `provider`),
/// and the input file's parent directory.
pub(crate) fn search_paths(input: &Path) -> Vec<PathBuf> {
  let mut paths = Vec::new();

  if let Ok(core_path) = env::var("ZO_CORE_PATH") {
    paths.push(PathBuf::from(core_path));
  } else {
    paths.extend(zo_host_paths::existing_lib_dirs(
      zo_host_paths::SYSTEM_PACK_ROOTS,
    ));
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

    process::exit(EXIT_CODE_ERROR);
  }

  match std::fs::read_to_string(path) {
    Ok(c) => c,
    Err(error) => {
      eprintln!("Error reading file {}: {error}", path.display());

      process::exit(EXIT_CODE_ERROR);
    }
  }
}

// TODO: add `check` — scan, parse, analyze and report.
// TODO: add `fmt` — format program.

/// Represents a [`Cmd`] enumeration.
#[derive(Debug, Subcommand)]
pub(crate) enum Cmd {
  /// build a program into an executable.
  Build(build::Build),
  /// read eval print and loop a program (not implemented yet).
  Repl(repl::Repl),
  /// build and run a program.
  Run(run::Run),
  /// compile and run test functions.
  Test(test::Test),
}
