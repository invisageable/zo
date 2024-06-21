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
  /// read a file, line or input.
  Reading(reading::Reading),
  /// tokenize bytes into `tokens`.
  Tokenizing(tokenizing::Tokenizing),
  /// parse tokens into `ast`.
  Parsing(parsing::Parsing),
  /// analyze an `ast`.
  Analyzing(analyzing::Analyzing),
  /// generate `bytecode` from `ast`
  Generating(generating::Generating),
  /// build output file from `bytecode`
  Building(building::Building),
  /// interpret an `ast`.
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
