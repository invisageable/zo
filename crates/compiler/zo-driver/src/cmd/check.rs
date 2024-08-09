use crate::cmd::Execute;

use zo_compiler::compiler::Compiler;
use zo_compiler::event::Event;
use zo_compiler::phase::analyzing::Analyzing;
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

/// The `check` command.
#[derive(Parser)]
pub(crate) struct Check {
  #[clap(short, long, default_value = "false")]
  verbose: bool,
  #[clap(short, long)]
  input: SmolStr,
  #[clap(short, long, default_value = "wasm")]
  backend: Backend,
  #[clap(short, long, default_value = "false")]
  profile: bool,
}

impl Check {
  /// Executes the `check` command.
  #[inline]
  fn check(&self) -> Result<()> {
    self.checking()
  }

  /// Checks a program.
  fn checking(&self) -> Result<()> {
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
    ]);

    match compiler.compile()? {
      Event::Ast(_) => Ok(()),
      event => Err(error::internal::expected_event(event)),
    }
  }
}

impl Execute for Check {
  #[inline]
  fn exec(&self) {
    match self.check() {
      Ok(_) => std::process::exit(EXIT_SUCCESS),
      Err(_) => std::process::exit(EXIT_FAILURE),
    }
  }
}
