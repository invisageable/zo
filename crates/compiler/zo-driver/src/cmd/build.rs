use super::Execute;

use zo_reporter::Result;
use zo_session::backend::Backend;
use zo_session::session::Session;
use zo_session::settings::Settings;

use swisskit::global::{EXIT_FAILURE, EXIT_SUCCESS};

use clap::Parser;

use smol_str::SmolStr;

/// The `build` command.
#[derive(Parser)]
pub(crate) struct Build {
  /// The pathname of an input.
  ///
  /// `--input <pathname>`.
  #[clap(short, long)]
  input: SmolStr,
  /// The code generation backend.
  #[clap(short, long, default_value = "wasm")]
  backend: Backend,
  /// The profiler flag.
  #[clap(short, long, default_value = "false")]
  profile: bool,
}

impl Build {
  /// Runs the build command.
  #[inline]
  fn build(&self) -> Result<()> {
    self.building()
  }

  /// Builds the program.
  fn building(&self) -> Result<()> {
    let mut _session = Session {
      settings: Settings {
        input: self.input.to_owned(),
        backend: self.backend.to_owned(),
        profile: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(
          self.profile,
        )),
        ..Default::default()
      },
      ..Default::default()
    };

    Ok(())
  }
}

impl Execute for Build {
  #[inline]
  fn exec(&self) {
    match self.build() {
      Ok(_) => std::process::exit(EXIT_SUCCESS),
      Err(_) => std::process::exit(EXIT_FAILURE),
    }
  }
}
