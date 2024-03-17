use crate::cmd::Handle;

use zhoo_compiler::compiler::Compiler;
use zhoo_compiler::phase::analyzing::Analyzing;
use zhoo_compiler::phase::parsing::Parsing;
use zhoo_compiler::phase::reading::Reading;
use zhoo_compiler::phase::tokenizing::Tokenizing;
use zhoo_compiler::phase::Phase;
use zhoo_session::session::Session;
use zhoo_session::settings::Settings;

use zo_core::mpsc::channel;
use zo_core::{Result, EXIT_FAILURE, EXIT_SUCCESS};

use clap::Parser;
use smol_str::SmolStr;

#[derive(Parser)]
pub(crate) struct Check {
  #[clap(short, long, default_value = "false")]
  verbose: bool,
  #[clap(short, long)]
  input: SmolStr,
  #[clap(short, long, default_value = "wasm")]
  backend: SmolStr,
}

impl Check {
  #[inline]
  fn check(&self) -> Result<()> {
    self.checking()
  }

  fn checking(&self) -> Result<()> {
    let mut session = Session {
      settings: Settings {
        input: self.input.to_owned(),
        backend: self.backend.to_owned().into(),
        ..Default::default()
      },
      ..Default::default()
    };

    let (rx_reading, tx_reading) = channel::bounded(channel::CAPACITY);
    let (rx_tokenizing, tx_tokenizing) = channel::bounded(channel::CAPACITY);
    let (rx_parsing, tx_parsing) = channel::bounded(channel::CAPACITY);
    let (rx_analyzing, tx_analyzing) = channel::bounded(channel::CAPACITY);

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
      }));

    compiler.compile(&mut session)?;

    compiler.finish(tx_analyzing).map(|_| {
      println!("finish.");
    })
  }
}

impl Handle for Check {
  #[inline]
  fn handle(&self) {
    match self.check() {
      Ok(_) => std::process::exit(EXIT_SUCCESS),
      Err(_) => std::process::exit(EXIT_FAILURE),
    }
  }
}
