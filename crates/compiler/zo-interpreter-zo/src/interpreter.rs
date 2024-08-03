use zo_ast::ast::{
  Ast, BinOp, BinOpKind, Expr, ExprKind, Lit, LitKind, Stmt, StmtKind, UnOp,
};

use zo_interner::interner::symbol::Symbol;
use zo_interner::interner::Interner;
use zo_reporter::reporter::Reporter;
use zo_reporter::Result;
use zo_session::session::Session;
use zo_value::value::{Value, ValueKind};

use swisskit::span::Span;

/// The representation of an interpreter.
struct Interpreter<'ast> {
  /// An interner — see also [`Interner`] for more information.
  interner: &'ast mut Interner,
  /// A reporter — see also [`Reporter`] for more information.
  reporter: &'ast Reporter,
}

impl<'ast> Interpreter<'ast> {
  /// Creates a new interpreter.
  #[inline]
  fn new(interner: &'ast mut Interner, reporter: &'ast Reporter) -> Self {
    Self { interner, reporter }
  }

  /// Evaluates an AST.
  fn interpret(&mut self, ast: &Ast) -> Result<Value> {
    let mut value = Value::UNIT;

    for stmt in ast.iter() {
      value = match self.interpret_stmt(stmt) {
        Ok(value) => value,
        Err(error) => self.reporter.raise(error),
      };
    }

    Ok(value)
  }

  /// Evaluates a statement.
  fn interpret_stmt(&mut self, stmt: &Stmt) -> Result<Value> {
    match &stmt.kind {
      StmtKind::Expr(expr) => self.interpret_stmt_expr(expr),
    }
  }

  /// Evaluates an expression statement.
  fn interpret_stmt_expr(&mut self, expr: &Expr) -> Result<Value> {
    self.interpret_expr(expr)
  }

  /// Evaluates an expression.
  fn interpret_expr(&mut self, expr: &Expr) -> Result<Value> {
    match &expr.kind {
      ExprKind::Lit(lit) => self.interpret_expr_lit(lit),
      ExprKind::UnOp(unop, rhs) => {
        self.interpret_expr_unop(unop, rhs, expr.span)
      }
      ExprKind::BinOp(binop, lhs, rhs) => {
        self.interpret_expr_binop(binop, lhs, rhs, expr.span)
      }
      _ => todo!(),
    }
  }

  /// Evaluates a literal expression.
  fn interpret_expr_lit(&mut self, lit: &Lit) -> Result<Value> {
    match &lit.kind {
      LitKind::Int(sym, _) => self.interpret_expr_lit_int(sym, lit.span),
      LitKind::Float(sym) => self.interpret_expr_lit_float(sym, lit.span),
      _ => todo!(),
    }
  }

  /// Evaluates an integer literal expression.
  fn interpret_expr_lit_int(
    &mut self,
    sym: &Symbol,
    span: Span,
  ) -> Result<Value> {
    let int = self.interner.lookup_int(**sym as usize);

    Ok(Value::int(int, span))
  }

  /// Evaluates a float literal expression.
  fn interpret_expr_lit_float(
    &mut self,
    sym: &Symbol,
    span: Span,
  ) -> Result<Value> {
    let float = self.interner.lookup_float(**sym as usize);

    Ok(Value::float(float, span))
  }

  /// Evaluates an unary operation expression.
  fn interpret_expr_unop(
    &mut self,
    unop: &UnOp,
    rhs: &Expr,
    span: Span,
  ) -> Result<Value> {
    todo!()
  }

  /// Evaluates a binary operation expression.
  fn interpret_expr_binop(
    &mut self,
    binop: &BinOp,
    lhs: &Expr,
    rhs: &Expr,
    span: Span,
  ) -> Result<Value> {
    let lhs = self.interpret_expr(lhs)?;
    let rhs = self.interpret_expr(rhs)?;

    match (&lhs.kind, &rhs.kind) {
      (ValueKind::Int(lhs), ValueKind::Int(rhs)) => {
        self.interpret_expr_binop_int(binop, lhs, rhs, span)
      }
      (ValueKind::Float(lhs), ValueKind::Float(rhs)) => {
        self.interpret_expr_binop_float(binop, lhs, rhs, span)
      }
      _ => todo!(),
    }
  }

  /// Evaluates a binary operation expression for integer.
  fn interpret_expr_binop_int(
    &mut self,
    binop: &BinOp,
    lhs: &i64,
    rhs: &i64,
    span: Span,
  ) -> Result<Value> {
    match binop.kind {
      BinOpKind::Add => Ok(Value::int(lhs + rhs, span)),
      BinOpKind::Sub => Ok(Value::int(lhs - rhs, span)),
      BinOpKind::Mul => Ok(Value::int(lhs * rhs, span)),
      BinOpKind::Div => Ok(Value::int(lhs / rhs, span)),
      _ => todo!(),
    }
  }

  /// Evaluates a binary operation expression for float.
  fn interpret_expr_binop_float(
    &mut self,
    binop: &BinOp,
    lhs: &f64,
    rhs: &f64,
    span: Span,
  ) -> Result<Value> {
    match binop.kind {
      BinOpKind::Add => Ok(Value::float(lhs + rhs, span)),
      BinOpKind::Sub => Ok(Value::float(lhs - rhs, span)),
      BinOpKind::Mul => Ok(Value::float(lhs * rhs, span)),
      BinOpKind::Div => Ok(Value::float(lhs / rhs, span)),
      _ => todo!(),
    }
  }
}

/// Evaluates an AST — see also [`Interpreter::interpret`].
///
/// #### examples.
///
/// ```
/// use zo_ast::ast::Ast;
/// use zo_interpreter_zo::interpreter;
/// use zo_session::session::Session;
/// use zo_value::value::Value;
///
/// let mut session = Session::default();
/// let ast = Ast::new();
/// let value = interpreter::interpret(&mut session, &ast);
///
/// assert_eq!(value, Value::UNIT);
/// ```
pub fn interpret(session: &mut Session, ast: &Ast) -> Result<Value> {
  Interpreter::new(&mut session.interner, &session.reporter).interpret(ast)
}
