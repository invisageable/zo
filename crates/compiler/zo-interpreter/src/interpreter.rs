use zo_ast::ast::Ast;
use zo_reporter::Result;
use zo_session::backend::Backend;
use zo_session::session::Session;
use zo_value::value::Value;

use zo_interpreter_clif as clif;
use zo_interpreter_zo as zo;

/// The representation of a interpreter.
struct Interpreter;
impl Interpreter {
  /// Creates a new interpreter.
  fn interpret(&mut self, session: &mut Session, ast: &Ast) -> Result<Value> {
    match session.settings.backend {
      Backend::Clif => clif::interpreter::interpret(session, ast),
      Backend::Zo => zo::interpreter::interpret(session, ast),
      _ => panic!(),
    }
  }
}

/// Executes a program and returns the value.
///
/// See also [`Interpreter::interpret`].
pub fn interpret(session: &mut Session, ast: &Ast) -> Result<Value> {
  Interpreter.interpret(session, ast)
}
