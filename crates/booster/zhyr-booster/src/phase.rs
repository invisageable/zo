pub mod building;
pub mod generating;
pub mod parsing;
pub mod reading;

use zhoo_session::session::Session;

use zo_core::Result;

pub trait Process: std::fmt::Debug {
  fn process(&self, session: &mut Session) -> Result<()>;
}

#[derive(Clone, Debug)]
pub enum Phase {
  Reading(reading::Reading),
  Parsing(parsing::Parsing),
  Generating(generating::Generating),
  Building(building::Building),
}
