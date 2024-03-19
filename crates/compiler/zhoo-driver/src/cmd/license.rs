use crate::cmd::Handle;

use zo_core::{Result, EXIT_FAILURE, EXIT_SUCCESS};

use clap::Parser;

#[derive(Parser)]
#[clap(about = "Print the compiler licences")]
pub(crate) struct License;

impl License {
  #[inline]
  fn license(&self) -> Result<()> {
    self.licensing()
  }

  fn licensing(&self) -> Result<()> {
    let paragraphs = include_str!("../../../../../.github/LICENSE-APACHE")
      .split("\n\n")
      .collect::<Vec<_>>();

    for paragraph in &paragraphs {
      println!("\n{paragraph}");
    }

    println!();

    let paragraphs = include_str!("../../../../../.github/LICENSE-MIT")
      .split("\n\n")
      .collect::<Vec<_>>();

    for paragraph in &paragraphs {
      println!("\n{paragraph}");
    }

    println!();

    Ok(())
  }
}

impl Handle for License {
  #[inline]
  fn handle(&self) {
    match self.license() {
      Ok(_) => std::process::exit(EXIT_SUCCESS),
      Err(_) => std::process::exit(EXIT_FAILURE),
    }
  }
}
