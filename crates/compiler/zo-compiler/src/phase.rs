pub mod analyzing;
pub mod building;
pub mod generating;
pub mod interpreting;
pub mod parsing;
pub mod reading;
pub mod tokenizing;

use super::event::Event;

use zo_reporter::Result;
use zo_session::session::Session;

use smol_str::{SmolStr, ToSmolStr};

/// The behavior of a phase to process.
pub trait Process {
  /// Runs the phase processing.
  fn process(&self, session: &mut Session, event: Event) -> Result<Event>;
}

/// The representation of a compiler's phase.
#[derive(Clone, Copy, Debug)]
pub enum Phase {
  /// The reading phase.
  Reading(reading::Reading),
  /// The lexical analysis phase.
  Tokenizing(tokenizing::Tokenizing),
  /// The syntax analysis phase.
  Parsing(parsing::Parsing),
  /// The semantic analysis phase.
  Analyzing(analyzing::Analyzing),
  /// The code generation phase.
  Generating(generating::Generating),
  /// The build phase.
  Building(building::Building),
  /// The interpretation phase.
  Interpreting(interpreting::Interpreting),
}

impl From<&Phase> for SmolStr {
  #[inline]
  fn from(phase: &Phase) -> Self {
    phase.to_smolstr()
  }
}

impl std::fmt::Display for Phase {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Reading(phase) => write!(f, "{phase}"),
      Self::Tokenizing(phase) => write!(f, "{phase}"),
      Self::Parsing(phase) => write!(f, "{phase}"),
      Self::Analyzing(phase) => write!(f, "{phase}"),
      Self::Generating(phase) => write!(f, "{phase}"),
      Self::Building(phase) => write!(f, "{phase}"),
      Self::Interpreting(phase) => write!(f, "{phase}"),
    }
  }
}
