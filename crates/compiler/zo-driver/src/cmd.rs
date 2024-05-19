mod build;
mod interpret;

use clap::{Parser, Subcommand};

pub(crate) trait Handle {
  fn handle(&self);
}

#[derive(Parser)]
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
    }
  }
}

#[derive(Subcommand)]
pub(crate) enum Command {
  Build(build::Build),
  Interpret(interpret::Interpret),
}
