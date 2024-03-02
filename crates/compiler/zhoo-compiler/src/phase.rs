//! ...

pub mod analyzing;
pub mod building;
pub mod generating;
pub mod interpreting;
pub mod linking;
pub mod parsing;
pub mod reading;
pub mod tokenizing;

use zhoo_session::session::Session;

use zo_core::Result;

pub trait Process: std::fmt::Debug {
  fn process(&self, session: &mut Session) -> Result<()>;
}

#[derive(Debug)]
pub enum Phase {
  Reading(reading::Reading),
  Tokenizing(tokenizing::Tokenizing),
  Parsing(parsing::Parsing),
  Analyzing(analyzing::Analyzing),
  Generating(generating::Generating),
  Building(building::Building),
  Linking(linking::Linking),
  Interpreting(interpreting::Interpreting),
}
