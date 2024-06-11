//! ...

use crate::cmd::Handle;

use clap::Parser;

use zo_core::{Result, EXIT_FAILURE, EXIT_SUCCESS};
use zo_session::session::Session;
use zo_session::settings::Settings;

use smol_str::SmolStr;

#[derive(Parser)]
pub(crate) struct Repl {
  #[clap(short, long, default_value = "false")]
  verbose: bool,
  #[clap(short, long)]
  input: SmolStr,
  #[clap(short, long, default_value = "wasm")]
  backend: SmolStr,
  #[clap(short, long, default_value = "false")]
  release: bool,
  #[clap(short, long, default_value = "false")]
  profile: bool,
}

impl Repl {
  fn repl(&self) -> Result<()> {
    self.repling()
  }

  fn repling(&self) -> Result<()> {
    let mut session = Session {
      settings: Settings {
        input: self.input.to_owned(),
        backend: self.backend.to_owned().into(),
        profile: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(
          self.profile,
        )),
        verbose: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(
          self.verbose,
        )),
      },
      ..Default::default()
    };

    session.open();

    Ok(())
  }
}

impl Handle for Repl {
  #[inline]
  fn handle(&self) {
    match self.repl() {
      Ok(_) => std::process::exit(EXIT_SUCCESS),
      Err(_) => std::process::exit(EXIT_FAILURE),
    }
  }
}
