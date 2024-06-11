//! ...

pub mod analyzing;
pub mod building;
pub mod generating;
pub mod interpreting;
pub mod parsing;
pub mod reading;
pub mod tokenizing;

use zo_session::session::Session;

use zo_core::Result;

use smol_str::{SmolStr, ToSmolStr};

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
  Interpreting(interpreting::Interpreting),
}

impl std::fmt::Display for Phase {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      Self::Reading(kind) => write!(f, "{kind}"),
      Self::Tokenizing(kind) => write!(f, "{kind}"),
      Self::Parsing(kind) => write!(f, "{kind}"),
      Self::Analyzing(kind) => write!(f, "{kind}"),
      Self::Generating(kind) => write!(f, "{kind}"),
      Self::Building(kind) => write!(f, "{kind}"),
      Self::Interpreting(kind) => write!(f, "{kind}"),
    }
  }
}

impl From<&Phase> for SmolStr {
  fn from(phase: &Phase) -> Self {
    SmolStr::new_inline(&phase.to_smolstr())
  }
}
