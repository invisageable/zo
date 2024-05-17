use zo_ast::ast::{Expr, ExprKind, Lit, LitKind};
use zo_value::value::Value;

pub struct Interpreter;

impl Interpreter {
  #[inline]
  pub fn new() -> Self {
    Self
  }

  pub fn interpret(&mut self, exprs: &[Expr]) -> Value {
    let mut value = Value::UNIT;

    for expr in exprs {
      value = self.interpret_expr(expr);
    }

    value
  }

  fn interpret_expr(&mut self, expr: &Expr) -> Value {
    match &expr.kind {
      ExprKind::Lit(lit) => self.interpret_expr_lit(lit),
      _ => todo!(),
    }
  }

  fn interpret_expr_lit(&mut self, lit: &Lit) -> Value {
    match &lit.kind {
      LitKind::Int(int) => self.interpret_expr_lit_int(int),
      LitKind::Float(float) => self.interpret_expr_lit_float(float),
      _ => todo!(),
    }
  }

  fn interpret_expr_lit_int(&mut self, int: &i64) -> Value {
    Value::int(*int)
  }

  fn interpret_expr_lit_float(&mut self, float: &f64) -> Value {
    Value::float(*float)
  }
}

pub fn interpret(interpreter: &mut Interpreter, exprs: &[Expr]) -> Value {
  interpreter.interpret(exprs)
}
