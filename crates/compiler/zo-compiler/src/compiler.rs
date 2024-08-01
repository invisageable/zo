use super::phase::{Phase, Process};

use zo_reporter::Result;
use zo_session::session::Session;

/// The compiler of the `zo` programming language.
///
/// #### compiler phases.
///
/// 1. Reading.
/// 2. Tokenizing.
/// 3. Parsing.
/// 4. Analyzing.
/// 5. Generating.
/// 6. Building.
/// 7. Interpreting.
#[derive(Debug)]
pub struct Compiler<const L: usize> {
  /// The compiler's phases.
  phases: [Phase; L],
}

impl<const L: usize> Compiler<L> {
  /// Creates a new compiler from phases.
  #[inline]
  pub fn new(phases: [Phase; L]) -> Self {
    Self { phases }
  }

  /// Compiles a program from a session by running each compiler's phases.
  pub fn compile(&self, session: &mut Session) -> Result<()> {
    self.phases.iter().try_fold((), |_, p| {
      session.with_timing(p, |session| match p {
        Phase::Reading(reading) => reading.process(session),
        Phase::Tokenizing(tokenizing) => tokenizing.process(session),
        Phase::Parsing(parsing) => parsing.process(session),
        Phase::Analyzing(analyzing) => analyzing.process(session),
        Phase::Generating(generating) => generating.process(session),
        Phase::Building(building) => building.process(session),
        Phase::Interpreting(interpreting) => interpreting.process(session),
      })
    })
  }
}

impl<const L: usize> From<[Phase; L]> for Compiler<L> {
  fn from(phases: [Phase; L]) -> Self {
    Self::new(phases)
  }
}
