use super::Handle;

use zhoo_session::settings::Settings;
use zhyr_booster::booster::Booster;
use zhyr_booster::phase::building::Building;
use zhyr_booster::phase::generating::Generating;
use zhyr_booster::phase::parsing::Parsing;
use zhyr_booster::phase::reading::Reading;
use zhyr_booster::phase::Phase;

use zhoo_session::session::Session;

use zo_core::mpsc::channel;
use zo_core::{Result, EXIT_FAILURE, EXIT_SUCCESS};

use clap::Parser;
use smol_str::SmolStr;

#[derive(Parser)]
pub struct Boost {
  #[clap(short, long, default_value = "false")]
  pub verbose: bool,
  #[clap(short, long)]
  pub input: SmolStr,
  #[clap(short, long, default_value = "py")]
  pub backend: SmolStr,
  #[clap(short, long, default_value = "false")]
  pub profile: bool,
}

impl Boost {
  #[inline]
  fn boost(&self) -> Result<()> {
    self.boosting()
  }

  #[inline]
  fn boosting(&self) -> Result<()> {
    let mut session = Session {
      settings: Settings {
        input: self.input.to_owned(),
        backend: self.backend.to_owned().into(),
        profile: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(
          self.profile.to_owned(),
        )),
        verbose: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(
          self.verbose.to_owned(),
        )),
      },
      ..Default::default()
    };

    let (rx_reading, tx_reading) = channel::bounded(channel::CAPACITY);
    let (rx_parsing, tx_parsing) = channel::bounded(channel::CAPACITY);
    let (rx_generating, tx_generating) = channel::bounded(channel::CAPACITY);
    let (rx_building, tx_building) = channel::bounded(channel::CAPACITY);

    let booster = Booster::new()
      .with_phase(Phase::Reading(Reading { rx: rx_reading }))
      .with_phase(Phase::Parsing(Parsing {
        rx: rx_parsing,
        tx: tx_reading,
      }))
      .with_phase(Phase::Generating(Generating {
        rx: rx_generating,
        tx: tx_parsing,
      }))
      .with_phase(Phase::Building(Building {
        rx: rx_building,
        tx: tx_generating,
      }));

    booster.compile(&mut session)?;

    booster.finish(tx_building).map(|_| {
      println!("finish.");
    })
  }
}

impl Handle for Boost {
  #[inline]
  fn handle(&self) {
    match self.boost() {
      Ok(_) => std::process::exit(EXIT_SUCCESS),
      Err(_) => std::process::exit(EXIT_FAILURE),
    }
  }
}
