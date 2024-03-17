mod build;
mod check;
mod license;
mod print;
mod version;

use clap::{Parser, Subcommand};

pub(crate) trait Handle {
  fn handle(&self);
}

#[derive(Parser)]
#[clap(about, author, name = "\nzhoo")]
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
      Command::Check(ref command) => command.handle(),
      Command::License(ref command) => command.handle(),
      Command::Print(ref command) => command.handle(),
      Command::Version(ref command) => command.handle(),
    }
  }
}

#[derive(Subcommand)]
pub(crate) enum Command {
  Build(build::Build),
  Check(check::Check),
  License(license::License),
  Print(license::License),
  Version(version::Version),
}
