use super::Execute;

use zo_compiler::compiler::Compiler;
use zo_compiler::event::Event;
use zo_compiler::phase::analyzing::Analyzing;
use zo_compiler::phase::building::Building;
use zo_compiler::phase::generating::Generating;
use zo_compiler::phase::parsing::Parsing;
use zo_compiler::phase::reading::Reading;
use zo_compiler::phase::tokenizing::Tokenizing;
use zo_compiler::phase::Phase;
use zo_reporter::{error, Result};
use zo_session::backend::Backend;
use zo_session::session::SESSION;
use zo_session::settings::Settings;

use swisskit::global::{EXIT_FAILURE, EXIT_SUCCESS};

use clap::Parser;
use smol_str::SmolStr;

/// The `build` command.
#[derive(Parser)]
pub(crate) struct Build {
  /// The pathname of an input.
  ///
  /// #### usage.
  ///
  /// `--input <pathname>`.
  #[clap(short, long)]
  input: SmolStr,
  /// The code generation backend.
  /// Default is `wasm`.
  ///
  /// #### usage.
  ///
  /// `--backend <backend>`.
  #[clap(short, long, default_value = "wasm")]
  backend: Backend,
  /// The profiler flag.
  ///
  /// #### usage.
  ///
  /// `--profile`.
  #[clap(short, long, default_value = "false")]
  profile: bool,
}

impl Build {
  /// Executes the `build` command.
  #[inline]
  fn build(&self) -> Result<()> {
    self.building()
  }

  /// Builds the program.
  fn building(&self) -> Result<()> {
    let session = std::sync::Arc::clone(&SESSION);
    let mut session = session.lock().unwrap();

    session.with_settings(Settings {
      input: self.input.clone(),
      backend: self.backend,
      profile: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(
        self.profile,
      )),
      ..Default::default()
    });

    drop(session);

    // phases will be execute in order.
    let compiler = Compiler::new([
      Phase::Reading(Reading),
      Phase::Tokenizing(Tokenizing),
      Phase::Parsing(Parsing),
      Phase::Analyzing(Analyzing),
      Phase::Generating(Generating),
      Phase::Building(Building),
    ]);

    match compiler.compile()? {
      Event::Output(output) => {
        println!("{output}");
        Ok(())
      }
      event => Err(error::internal::expected_event(event)),
    }
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
