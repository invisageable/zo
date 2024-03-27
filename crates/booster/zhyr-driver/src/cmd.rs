mod boost;

use clap::{Parser, Subcommand};

pub trait Handle {
  fn handle(&self);
}

#[derive(Parser)]
#[clap(about, author, name = "\nzhyr")]
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
      Command::Boost(ref command) => command.handle(),
    }
  }
}

#[derive(Subcommand)]
pub(crate) enum Command {
  Boost(boost::Boost),
}
