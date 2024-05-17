use zo_ast::ast::Expr;
use zo_value::value::Value;

pub struct Interpreter;

impl Interpreter {
  #[inline]
  pub fn new() -> Self {
    Self
  }

  pub fn interpret(&mut self, _exprs: &[Expr]) -> Value {
    todo!()
  }
}

pub fn interpret(interpreter: &mut Interpreter, exprs: &[Expr]) -> Value {
  interpreter.interpret(exprs)
}
