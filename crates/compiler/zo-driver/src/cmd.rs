pub mod build;
pub mod repl;
pub mod run;

use clap::{Parser, Subcommand};

/// A behavior to execute a command.
pub(crate) trait Execute {
  /// Executes a command.
  fn exec(&self);
}

/// The representation of a command.
#[derive(Parser)]
#[clap(about, author, name = "\nzo")]
pub(crate) struct Cmd {
  /// A comand.
  #[clap(subcommand)]
  command: Command,
}

impl Cmd {
  /// Creates a new cmd.
  #[inline]
  pub fn run(&mut self) {
    self.cmd()
  }

  /// Executes a command.
  #[inline]
  fn cmd(&mut self) {
    match self.command {
      Command::Build(ref command) => command.exec(),
      Command::Run(ref command) => command.exec(),
      _ => unimplemented!(),
    }
  }
}

/// The representation of a command kind.
#[derive(Subcommand)]
pub(crate) enum Command {
  /// Builds a program.
  Build(build::Build),
  /// Runs a program.
  Run(run::Run),
  /// Reads eval print and loop a program.
  Repl(repl::Repl),
}
