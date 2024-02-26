use super::phase::{Phase, Process};

use zhoo_session::session::Session;
use zo_core::mpsc::receiver::Receiver;
use zo_core::Result;

#[derive(Debug, Default)]
pub struct Compiler {
  phases: Vec<Phase>,
}

impl Compiler {
  #[inline]
  pub fn new() -> Self {
    Self {
      phases: Vec::with_capacity(0usize),
    }
  }

  #[inline]
  pub fn add_phase(mut self, phase: Phase) -> Self {
    self.phases.push(phase);
    self
  }

  pub fn compile(&self, session: &mut Session) -> Result<()> {
    self.phases.iter().try_fold((), |_, phase| match phase {
      Phase::Reading(phase) => phase.process(session),
      Phase::Tokenizing(phase) => phase.process(session),
      Phase::Parsing(phase) => phase.process(session),
      Phase::Analyzing(phase) => phase.process(session),
      Phase::Generating(phase) => phase.process(session),
      Phase::Building(phase) => phase.process(session),
      _ => todo!(),
    })
  }

  pub fn finish<T>(&self, receiver: Receiver<T>) -> Result<T> {
    receiver.recv()
  }
}
