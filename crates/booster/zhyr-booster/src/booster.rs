use super::phase::{Phase, Process};

use zhoo_session::session::Session;

use zo_core::mpsc::receiver::Receiver;
use zo_core::Result;

#[derive(Debug, Default)]
pub struct Booster {
  phases: Vec<Phase>,
}

impl Booster {
  #[inline]
  pub fn new() -> Self {
    Self::default()
  }

  #[inline]
  pub fn with_phase(mut self, phase: Phase) -> Self {
    self.phases.push(phase);
    self
  }

  pub fn compile(&self, session: &mut Session) -> Result<()> {
    self.phases.iter().try_fold((), |_, phase| match phase {
      Phase::Reading(phase) => phase.process(session),
      Phase::Parsing(phase) => phase.process(session),
      Phase::Generating(phase) => phase.process(session),
      Phase::Building(phase) => phase.process(session),
    })
  }

  #[inline]
  pub fn finish<T>(&self, receiver: Receiver<T>) -> Result<T> {
    receiver.recv()
  }
}
