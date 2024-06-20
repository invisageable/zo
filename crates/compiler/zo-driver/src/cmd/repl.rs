//! ...

// todo #1 — use a better implementation instead of that ugly
// implementation of channel. it does the job done for now but in the
// future maybe, a better approach should be written.

use crate::cmd::Handle;

use zo_compiler::compiler::Compiler;
use zo_compiler::phase::analyzing::Analyzing;
use zo_compiler::phase::interpreting::Interpreting;
use zo_compiler::phase::parsing::Parsing;
use zo_compiler::phase::reading::Reading;
use zo_compiler::phase::tokenizing::Tokenizing;
use zo_compiler::phase::Phase;
use zo_session::session::Session;
use zo_session::settings::Settings;

use zo_core::mpsc::channel;
use zo_core::{Result, EXIT_FAILURE, EXIT_SUCCESS};

use clap::Parser;
use smol_str::SmolStr;

#[derive(Parser)]
pub(crate) struct Repl {
  #[clap(short, long, default_value = "false")]
  verbose: bool,
  #[clap(short, long, default_value = "wasm")]
  backend: SmolStr,
  #[clap(short, long, default_value = "false")]
  release: bool,
  #[clap(short, long, default_value = "false")]
  profile: bool,
}

impl Repl {
  #[inline]
  fn repl(&self) -> Result<()> {
    self.repling()
  }

  fn repling(&self) -> Result<()> {
    let mut session = Session {
      settings: Settings {
        backend: self.backend.to_owned().into(),
        profile: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(
          self.profile,
        )),
        verbose: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(
          self.verbose,
        )),
        interactive: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(
          true,
        )),
        ..Default::default()
      },
      ..Default::default()
    };

    session.open();

    loop {
      // todo #1.
      let (rx_reading, tx_reading) = channel::bounded(channel::CAPACITY);
      let (rx_tokenizing, tx_tokenizing) = channel::bounded(channel::CAPACITY);
      let (rx_parsing, tx_parsing) = channel::bounded(channel::CAPACITY);
      let (rx_analyzing, tx_analyzing) = channel::bounded(channel::CAPACITY);

      let (rx_interpreting, tx_interpreting) =
        channel::bounded(channel::CAPACITY);

      let compiler = Compiler::new()
        .add_phase(Phase::Reading(Reading { rx: rx_reading }))
        .add_phase(Phase::Tokenizing(Tokenizing {
          rx: rx_tokenizing,
          tx: tx_reading,
        }))
        .add_phase(Phase::Parsing(Parsing {
          rx: rx_parsing,
          tx: tx_tokenizing,
        }))
        .add_phase(Phase::Analyzing(Analyzing {
          rx: rx_analyzing,
          tx: tx_parsing,
        }))
        .add_phase(Phase::Interpreting(Interpreting {
          rx: rx_interpreting,
          tx: tx_analyzing,
        }));

      compiler.compile(&mut session)?;

      compiler.finish(tx_interpreting).map(|value| {
        println!("{value}");
        session.close();
      })?;
    }
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
