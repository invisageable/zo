pub mod build;
pub mod check;
pub mod license;
pub mod repl;
pub mod run;
pub mod version;

use clap::{Parser, Subcommand};

/// A behavior to execute a command.
pub(crate) trait Execute {
  /// Executes the command.
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
      Command::Check(ref command) => command.exec(),
      Command::License(ref command) => command.exec(),
      Command::Repl(ref command) => command.exec(),
      Command::Run(ref command) => command.exec(),
      Command::Version(ref command) => command.exec(),
    }
  }
}

/// The representation of a command kind.
#[derive(Subcommand)]
pub(crate) enum Command {
  /// Builds a program.
  Build(build::Build),
  /// Checks a program.
  Check(check::Check),
  /// Typewrites licenses.
  License(license::License),
  /// Runs a program.
  Run(run::Run),
  /// Reads eval print and loop a program.
  Repl(repl::Repl),
  /// Gets the `zo` version compiler.
  Version(version::Version),
}
