use crate::cmd::{Cmd, Handle};

use clap::Parser;

/// Represents a [`Driver`] instance.
#[derive(Debug, Parser)]
#[clap(
  name = "\nzo",
  about = "The zo Programming Language",
  author = "compilords"
)]
pub(crate) struct Driver {
  /// The commands that can be run be the [`Driver`].
  #[clap(subcommand)]
  cmd: Cmd,
}

impl Driver {
  /// Runs a command.
  pub(crate) fn run(self) {
    match self.cmd {
      Cmd::Build(ref cmd) => cmd.handle(),
      Cmd::Repl(ref cmd) => cmd.handle(),
      Cmd::Run(ref cmd) => cmd.handle(),
    }
  }
}
