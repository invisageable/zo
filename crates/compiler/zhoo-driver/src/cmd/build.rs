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

use zo_core::mpsc::channel;
use zo_core::Result;

use clap::Parser;

#[derive(Parser)]
pub(crate) struct Build {
  #[clap(short, long, default_value = "false")]
  pub verbose: bool,
  #[clap(short, long)]
  pub input: smol_str::SmolStr,
  #[clap(short, long, default_value = "wasm")]
  pub target: smol_str::SmolStr,
  #[clap(short, long, default_value = "false")]
  pub release: bool,
  #[clap(short, long, default_value = "false")]
  pub profile: bool,
}

impl Build {
  #[inline]
  pub fn compile(&self) -> Result<()> {
    self.compiling()
  }

  #[inline]
  fn compiling(&self) -> Result<()> {
    let mut session = Session {
      input: self.input.to_owned(),
      ..Default::default()
    };

    // todo(ivs): kind of ugly implementation of channel.
    // it does the job done for the moment but in the future maybe.
    // we will need to change this approach.
    let (rx_reading, tx_reading) = channel::bounded(1usize);
    let (rx_tokenizing, tx_tokenizing) = channel::bounded(1usize);
    let (rx_parsing, tx_parsing) = channel::bounded(1usize);
    let (rx_analyzing, tx_analyzing) = channel::bounded(1usize);
    let (rx_generating, tx_generating) = channel::bounded(1usize);
    let (rx_building, tx_building) = channel::bounded(1usize);

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
      Ok(_) => std::process::exit(0i32),
      Err(_) => std::process::exit(1i32),
    }
  }
}
