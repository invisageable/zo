use crate::cmd::Handle;

use zhoo_compiler::compiler::Compiler;
use zhoo_compiler::phase::analyzing::Analyzing;
use zhoo_compiler::phase::building::Building;
use zhoo_compiler::phase::generating::Generating;
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
#[clap(about = "Compile the current pack")]
pub(crate) struct Build {
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

impl Build {
  #[inline]
  pub fn compile(&self) -> Result<()> {
    self.compiling()
  }

  fn compiling(&self) -> Result<()> {
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

    // todo (ivs) — kind of ugly implementation of channel.
    // it does the job done for the moment but in the future maybe,
    // we will need to change this approach.
    let (rx_reading, tx_reading) = channel::bounded(channel::CAPACITY);
    let (rx_tokenizing, tx_tokenizing) = channel::bounded(channel::CAPACITY);
    let (rx_parsing, tx_parsing) = channel::bounded(channel::CAPACITY);
    let (rx_analyzing, tx_analyzing) = channel::bounded(channel::CAPACITY);
    let (rx_generating, tx_generating) = channel::bounded(channel::CAPACITY);
    let (rx_building, tx_building) = channel::bounded(channel::CAPACITY);

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
      .add_phase(Phase::Generating(Generating {
        rx: rx_generating,
        tx: tx_analyzing,
      }))
      .add_phase(Phase::Building(Building {
        rx: rx_building,
        tx: tx_generating,
      }));

    compiler.compile(&mut session)?;

    compiler.finish(tx_building).map(|_output| {
      println!("finish.");
    })
  }
}

impl Handle for Build {
  #[inline]
  fn handle(&self) {
    match self.compile() {
      Ok(_) => std::process::exit(EXIT_SUCCESS),
      Err(_) => std::process::exit(EXIT_FAILURE),
    }
  }
}
