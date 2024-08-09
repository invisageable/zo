use crate::cmd::Execute;

use zo_reporter::Result;

use swisskit::global::{EXIT_FAILURE, EXIT_SUCCESS};
use swisskit::typewriteln;

use clap::Parser;

/// The `license` command.
#[derive(Parser)]
pub(crate) struct License;
impl License {
  /// Executes the `license` command.
  #[inline]
  fn license(&self) -> Result<()> {
    self.licensing()
  }

  /// Typewrites licenses.
  fn licensing(&self) -> Result<()> {
    let paragraphs = include_str!("../../../../../LICENSE-APACHE")
      .split("\n\n")
      .collect::<Vec<_>>();

    for paragraph in paragraphs {
      typewriteln!(paragraph, std::time::Duration::from_millis(1));
    }

    println!();

    let paragraphs = include_str!("../../../../../LICENSE-MIT")
      .split("\n\n")
      .collect::<Vec<_>>();

    for paragraph in paragraphs {
      typewriteln!(paragraph, std::time::Duration::from_millis(1));
    }

    println!();

    Ok(())
  }
}

impl Execute for License {
  #[inline]
  fn exec(&self) {
    match self.license() {
      Ok(_) => std::process::exit(EXIT_SUCCESS),
      Err(_) => std::process::exit(EXIT_FAILURE),
    }
  }
}
