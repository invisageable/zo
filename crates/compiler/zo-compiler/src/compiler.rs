use super::phase::{Phase, Process};

use zo_ast::ast::Ast;
use zo_reporter::Result;
use zo_session::session::SESSION;
use zo_tokenizer::token::Token;

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
  pub fn compile(&self) -> Result<Event> {
    let session = SESSION.clone();
    let phase = std::sync::Arc::new(std::sync::Mutex::new(self.phases));
    let (tx, rx) = flume::unbounded();

    // the producer sends compiler phases to the consumer.
    let producer = std::thread::spawn(move || {
      let guard_phase = phase.lock().unwrap();

      for phase in *guard_phase {
        tx.send(phase).unwrap();
      }
    });

    // the consumer folds those compiler phases and pass the appropriate event.
    let consumer = std::thread::spawn(move || {
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
      .map(|on| {
        let session = SESSION.clone();
        let session = session.lock().unwrap();

        session.profile();
        drop(session);

        on
      })
      .unwrap()
  }
}

impl<const L: usize> From<[Phase; L]> for Compiler<L> {
  fn from(phases: [Phase; L]) -> Self {
    Self::new(phases)
  }
}

/// The representation of compiler's event.
#[derive(Debug)]
pub enum Event {
  Path(std::path::PathBuf),
  Bytes(Vec<u8>),
  Tokens(Vec<Token>),
  Ast(Ast),
  // Bytecode(Vec<u8>),
  // Value(Value),
  // Output(Output),
}

impl Event {
  /// Creates a new path event.
  #[inline]
  pub const fn path(path: std::path::PathBuf) -> Result<Self> {
    Ok(Event::Path(path))
  }

  /// Creates a new bytes event.
  #[inline]
  pub const fn bytes(bytes: Vec<u8>) -> Result<Self> {
    Ok(Event::Bytes(bytes))
  }

  /// Creates a new tokens event.
  #[inline]
  pub const fn tokens(tokens: Vec<Token>) -> Result<Self> {
    Ok(Event::Tokens(tokens))
  }

  /// Creates a new ast event.
  #[inline]
  pub const fn ast(ast: Ast) -> Result<Self> {
    Ok(Event::Ast(ast))
  }
}
