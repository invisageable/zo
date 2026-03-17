mod build;
mod init;

use clap::Subcommand;

pub(crate) trait Handle {
  /// Handles the execution of a command.
  fn handle(&self);
}

/// Represents a [`Cmd`] enumeration.
#[derive(Debug, Subcommand)]
pub(crate) enum Cmd {
  /// Builds a zo project.
  Build(build::Build),
  /// Initializes a new zo project.
  Init(init::Init),
}
