use super::event::Event;
use super::phase::{Phase, Process};

use zo_reporter::Result;
use zo_session::session::SESSION;

// todo(ivs) — #1.
//
// implements an internal error.

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
///
/// These phase will be handled depending of on the command.
///
/// For example the `build` command runs phases `1`, `2`, `3`, `4`, `5`, `6`.
/// Instead the `run` command executes phase `1`, `2`, `3`, `4`, `7`.
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
  pub fn compile(&self) -> Result<Event> {
    let session = SESSION.clone();
    let phase = std::sync::Arc::new(std::sync::Mutex::new(self.phases));
    let (tx, rx) = flume::unbounded();

    // the producer sends compiler phases to the consumer.
    let producer = std::thread::spawn(move || {
      // todo(ivs) — #1.
      let guard_phase = phase.lock().unwrap();

      for phase in *guard_phase {
        // todo(ivs) — #1.
        tx.send(phase).unwrap();
      }
    });

    // the consumer folds those compiler phases and pass the appropriate event.
    let consumer = std::thread::spawn(move || {
      // todo(ivs) — #1.
      let mut session = session.lock().unwrap();
      let input = session.settings.input.as_str();
      let event = Event::Path(input.into());

      rx.iter().try_fold(event, |event, p| {
        session.with_timing(&p, |session| match p {
          Phase::Reading(phase) => phase.process(session, event),
          Phase::Tokenizing(phase) => phase.process(session, event),
          Phase::Parsing(phase) => phase.process(session, event),
          Phase::Analyzing(phase) => phase.process(session, event),
          Phase::Generating(phase) => phase.process(session, event),
          Phase::Building(phase) => phase.process(session, event),
          Phase::Interpreting(phase) => phase.process(session, event),
        })
      })
    });

    // now we can handle the producer.
    producer.join().unwrap();

    // same with the consumer.
    consumer
      .join()
      .inspect(|_event| {
        let session = SESSION.clone();
        // todo(ivs) — #1.
        let session = session.lock().unwrap();

        session.profile();
        drop(session);
      })
      .unwrap()
  }
}

impl<const L: usize> From<[Phase; L]> for Compiler<L> {
  #[inline]
  fn from(phases: [Phase; L]) -> Self {
    Self::new(phases)
  }
}
