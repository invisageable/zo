use super::Execute;

use zo_compiler::compiler::Compiler;
use zo_compiler::event::Event;
use zo_compiler::phase::analyzing::Analyzing;
use zo_compiler::phase::interpreting::Interpreting;
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

/// The `run` command.
#[derive(Parser)]
pub(crate) struct Run {
  /// The pathname of an input.
  ///
  /// #### usage.
  ///
  /// `--input <pathname>`.
  #[clap(short, long)]
  input: SmolStr,
  /// The code generation backend.
  /// Default is `zo`.
  ///
  /// #### usage.
  ///
  /// `--backend <backend>`.
  #[clap(short, long, default_value = "zo")]
  backend: Backend,
  /// The profiler flag.
  ///
  /// #### usage.
  ///
  /// `--profile`.
  #[clap(short, long, default_value = "false")]
  profile: bool,
  /// The verbose flag.
  ///
  /// #### usage.
  ///
  /// `--verbose`.
  #[clap(short, long, default_value = "false")]
  verbose: bool,
}

impl Run {
  /// Executes the `run` command.
  #[inline]
  fn run(&self) -> Result<()> {
    self.running()
  }

  /// Interprets a program.
  fn running(&self) -> Result<()> {
    let session = std::sync::Arc::clone(&SESSION);
    let mut session = session.lock().unwrap();

    session.with_settings(Settings {
      input: self.input.clone(),
      backend: self.backend,
      profile: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(
        self.profile,
      )),
      verbose: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(
        self.verbose,
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
      Phase::Interpreting(Interpreting),
    ]);

    match compiler.compile()? {
      Event::Value(value) => {
        println!("{value}");
        Ok(())
      }
      event => Err(error::internal::expected_event(event)),
    }
  }
}

impl Execute for Run {
  #[inline]
  fn exec(&self) {
    match self.run() {
      Ok(_) => std::process::exit(EXIT_SUCCESS),
      Err(_) => std::process::exit(EXIT_FAILURE),
    }
  }
}
