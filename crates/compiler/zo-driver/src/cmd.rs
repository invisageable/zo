//! ...

mod build;
mod interpret;
mod repl;

use clap::{Parser, Subcommand};

pub(crate) trait Handle {
  fn handle(&self);
}

#[derive(Debug, Parser)]
#[clap(about, author, name = "\nzo")]
pub(crate) struct Cmd {
  #[clap(subcommand)]
  command: Command,
}

impl Cmd {
  #[inline]
  pub fn run(&mut self) {
    self.cmd()
  }

  #[inline]
  fn cmd(&mut self) {
    match self.command {
      Command::Build(ref command) => command.handle(),
      Command::Interpret(ref command) => command.handle(),
      Command::Repl(ref command) => command.handle(),
    }
  }
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
  /// build a program.
  Build(build::Build),
  /// interpret a program.
  Interpret(interpret::Interpret),
  /// read eval print and loop a program.
  Repl(repl::Repl),
}
