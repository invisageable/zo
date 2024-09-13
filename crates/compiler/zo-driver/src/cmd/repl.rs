use crate::cmd::Execute;

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

/// The `repl` command.
#[derive(Parser)]
pub(crate) struct Repl;

impl Repl {
  /// Executes the `repl` command.
  #[inline]
  fn repl(&self) -> Result<()> {
    self.repling()
  }

  /// Read-eval-print loop interactive mode.
  fn repling(&self) -> Result<()> {
    let session = std::sync::Arc::clone(&SESSION);
    let mut session = session.lock().unwrap();

    session.with_settings(Settings {
      backend: Backend::Zo,
      interactive: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(
        true,
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

    loop {
      match compiler.compile()? {
        Event::Value(value) => println!("{value}"),
        event => return Err(error::internal::expected_event(event)),
      }
    }
  }
}

impl Execute for Repl {
  #[inline]
  fn exec(&self) {
    match self.repl() {
      Ok(_) => std::process::exit(EXIT_SUCCESS),
      Err(_) => std::process::exit(EXIT_FAILURE),
    }
  }
}
