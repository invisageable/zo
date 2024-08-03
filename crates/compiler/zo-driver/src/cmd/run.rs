use super::Execute;

use zo_compiler::compiler::Compiler;
use zo_compiler::event::Event;
use zo_compiler::phase::analyzing::Analyzing;
use zo_compiler::phase::interpreting::Interpreting;
use zo_compiler::phase::parsing::Parsing;
use zo_compiler::phase::reading::Reading;
use zo_compiler::phase::tokenizing::Tokenizing;
use zo_compiler::phase::Phase;
use zo_reporter::Result;
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
}

impl Run {
  /// Executes the `run` command.
  #[inline]
  fn build(&self) -> Result<()> {
    self.building()
  }

  /// Interprets the program.
  fn building(&self) -> Result<()> {
    let session = std::sync::Arc::clone(&SESSION);
    let mut session = session.lock().unwrap();

    // am i legitime to unwrap here?
    session.with_settings(Settings {
      input: self.input.to_owned(),
      backend: self.backend.to_owned(),
      profile: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(
        self.profile,
      )),
      ..Default::default()
    });

    drop(session);

    // phases will be execute in fifo ordering.
    let compiler = Compiler::new([
      Phase::Reading(Reading),
      Phase::Tokenizing(Tokenizing),
      Phase::Parsing(Parsing),
      Phase::Analyzing(Analyzing),
      Phase::Interpreting(Interpreting),
    ]);

    match compiler.compile()? {
      Event::Value(value) => {
        println!("{value:?}");
        Ok(())
      }
      _ => panic!(),
    }
  }
}

impl Execute for Run {
  #[inline]
  fn exec(&self) {
    match self.build() {
      Ok(_) => std::process::exit(EXIT_SUCCESS),
      Err(_) => std::process::exit(EXIT_FAILURE),
    }
  }
}
