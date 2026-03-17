use crate::cmd::{Cmd, Handle};

use clap::Parser;

/// Represents a [`Driver`] instance.
#[derive(Debug, Parser)]
#[clap(
  name = "\nfret",
  about = "The fret package manager for zo",
  author = "compilords"
)]
pub(crate) struct Driver {
  /// The commands that can be run by the [`Driver`].
  #[clap(subcommand)]
  cmd: Cmd,
}

impl Driver {
  /// Runs a command.
  pub(crate) fn run(self) {
    match self.cmd {
      Cmd::Build(ref cmd) => cmd.handle(),
      Cmd::Init(ref cmd) => cmd.handle(),
    }
  }
}
