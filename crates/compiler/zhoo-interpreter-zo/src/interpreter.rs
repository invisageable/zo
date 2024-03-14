#![allow(dead_code)]

use zhoo_ast::ast::Program;
use zhoo_session::session::Session;

use zo_core::interner::Interner;
use zo_core::Result;

struct Interpreter<'program> {
  interner: &'program mut Interner,
}

impl<'program> Interpreter<'program> {
  fn new(interner: &'program mut Interner) -> Self {
    Self { interner }
  }

  fn interpret(&mut self, _program: &Program) -> Result<()> {
    todo!()
  }
}

/// ...
///
/// ## examples.
///
/// ```
/// ```
pub fn interpret(session: &mut Session, program: &Program) -> Result<()> {
  Interpreter::new(&mut session.interner).interpret(program)
}
