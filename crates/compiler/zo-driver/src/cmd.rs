mod build;
mod repl;
mod run;

use clap::Subcommand;

pub(crate) trait Handle {
  /// Handles the execution of a command.
  fn handle(&self);
}

/// Represents a [`Cmd`] enumeration.
#[derive(Debug, Subcommand)]
pub(crate) enum Cmd {
  /// builds a program.
  Build(build::Build),
  /// read eval print and loop a program.
  Repl(repl::Repl),
  /// runs a program.
  Run(run::Run),
}
