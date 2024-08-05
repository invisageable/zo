use super::scope::ScopeMap;

use zo_ast::ast::{
  Ast, BinOp, BinOpKind, Expr, ExprKind, Item, ItemKind, Lit, LitKind, Stmt,
  StmtKind, UnOp, UnOpKind, Var,
};

use zo_interner::interner::symbol::{Symbol, Symbolize};
use zo_interner::interner::Interner;
use zo_reporter::reporter::Reporter;
use zo_reporter::{error, Result};
use zo_session::session::Session;
use zo_value::value::{Value, ValueKind};

use swisskit::span::Span;

use smol_str::ToSmolStr;

/// The representation of an interpreter.
struct Interpreter<'ast> {
  /// A scope map — see also [`ScopeMap`].
  scope_map: ScopeMap,
  /// An interner — see also [`Interner`] for more information.
  interner: &'ast mut Interner,
  /// A reporter — see also [`Reporter`] for more information.
  reporter: &'ast Reporter,
}

impl<'ast> Interpreter<'ast> {
  /// Creates a new interpreter.
  #[inline]
  fn new(interner: &'ast mut Interner, reporter: &'ast Reporter) -> Self {
    Self {
      scope_map: ScopeMap::new(),
      interner,
      reporter,
    }
  }

  /// Evaluates an AST.
  fn interpret(&mut self, ast: &Ast) -> Result<Value> {
    let mut value = Value::UNIT;

    self.scope_map.scope_entry();

    for stmt in ast.iter() {
      value = match self.interpret_stmt(stmt) {
        Ok(value) => value,
        Err(error) => self.reporter.raise(error),
      };
    }

    self.scope_map.scope_exit();

    Ok(value)
  }

  /// Evaluates an item statement.
  fn interpret_stmt_item(&mut self, item: &Item) -> Result<Value> {
    match &item.kind {
      ItemKind::Var(var) => self.interpret_item_var(var),
    }
  }

  /// Evaluates a variable item.
  fn interpret_item_var(&mut self, var: &Var) -> Result<Value> {
    self.interpret_global_var(var)
  }

  /// Evaluates a global variable.
  fn interpret_global_var(&mut self, var: &Var) -> Result<Value> {
    let value = self.interpret_expr(&var.value)?;
    let name = *var.pattern.as_symbol();

    self.scope_map.add_var(name, value.clone())?;

    Ok(value)
  }

  /// Evaluates a statement.
  fn interpret_stmt(&mut self, stmt: &Stmt) -> Result<Value> {
    match &stmt.kind {
      StmtKind::Var(var) => self.interpret_stmt_var(var),
      StmtKind::Item(var) => self.interpret_stmt_item(var),
      StmtKind::Expr(expr) => self.interpret_stmt_expr(expr),
    }
  }

  /// Evaluates a local variable statement.
  fn interpret_stmt_var(&mut self, var: &Var) -> Result<Value> {
    self.interpret_local_var(var)
  }

  /// Evaluates a variable.
  fn interpret_local_var(&mut self, var: &Var) -> Result<Value> {
    let value = self.interpret_expr(&var.value)?;
    let name = *var.pattern.as_symbol();

    self.scope_map.add_var(name, value.clone())?;

    Ok(value)
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
      ExprKind::Assign(assignee, value) => {
        self.interpret_expr_assign(assignee, value)
      }
      ExprKind::AssignOp(binop, assignee, value) => {
        self.interpret_expr_assign_op(binop, assignee, value, expr.span)
      }
      ExprKind::Array(elmts) => self.interpret_expr_array(elmts, expr.span),
      ExprKind::ArrayAccess(indexed, index) => {
        self.interpret_expr_array_access(indexed, index, expr.span)
      }
      _ => todo!(),
    }
  }

  /// Evaluates a literal expression.
  fn interpret_expr_lit(&mut self, lit: &Lit) -> Result<Value> {
    match &lit.kind {
      LitKind::Int(sym, _) => self.interpret_expr_lit_int(sym, lit.span),
      LitKind::Float(sym) => self.interpret_expr_lit_float(sym, lit.span),
      LitKind::Ident(sym) => self.interpret_expr_lit_ident(sym, lit.span),
      LitKind::Bool(boolean) => self.interpret_expr_lit_bool(boolean, lit.span),
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

  /// Evaluates a identifier literal expression.
  fn interpret_expr_lit_ident(
    &mut self,
    sym: &Symbol,
    span: Span,
  ) -> Result<Value> {
    if let Some(var) = self.scope_map.var(sym) {
      return Ok(var.to_owned());
    } else if let Some(fun) = self.scope_map.fun(sym) {
      return Ok(fun.to_owned());
    }

    Err(error::eval::not_found_ident(span, *sym))
  }

  /// Evaluates a boolean literal expression.
  fn interpret_expr_lit_bool(
    &mut self,
    boolean: &bool,
    span: Span,
  ) -> Result<Value> {
    Ok(Value::bool(*boolean, span))
  }

  /// Evaluates an unary operation expression.
  fn interpret_expr_unop(
    &mut self,
    unop: &UnOp,
    rhs: &Expr,
    span: Span,
  ) -> Result<Value> {
    let value = self.interpret_expr(rhs)?;

    match unop.kind {
      UnOpKind::Neg => self.interpret_expr_unop_neg(value, span),
      UnOpKind::Not => self.interpret_expr_unop_not(value, span),
    }
  }

  /// Evaluates a negative unary operation expression.
  fn interpret_expr_unop_neg(
    &mut self,
    rhs: Value,
    span: Span,
  ) -> Result<Value> {
    match rhs.kind {
      ValueKind::Int(int) => Ok(Value::int(-int, span)),
      ValueKind::Float(float) => Ok(Value::float(-float, span)),
      _ => Err(error::eval::unknown_unop(span, rhs.to_smolstr())),
    }
  }

  /// Evaluates a logical NOT unary operation expression
  fn interpret_expr_unop_not(
    &mut self,
    rhs: Value,
    span: Span,
  ) -> Result<Value> {
    Ok(Value::bool(!rhs.as_bool(), span))
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
      (ValueKind::Bool(lhs), ValueKind::Bool(rhs)) => {
        self.interpret_expr_binop_bool(binop, lhs, rhs, span)
      }
      _ => Err(error::eval::unknown_binop(span, *binop)),
    }
  }

  /// Evaluates a binary operation expression for integers.
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
      BinOpKind::Rem => Ok(Value::int(lhs % rhs, span)),
      BinOpKind::Shl => Ok(Value::int(lhs << rhs, span)),
      BinOpKind::Shr => Ok(Value::int(lhs >> rhs, span)),
      BinOpKind::Lt => Ok(Value::bool(lhs < rhs, span)),
      BinOpKind::Gt => Ok(Value::bool(lhs > rhs, span)),
      BinOpKind::Le => Ok(Value::bool(lhs <= rhs, span)),
      BinOpKind::Ge => Ok(Value::bool(lhs >= rhs, span)),
      BinOpKind::Eq => Ok(Value::bool(lhs == rhs, span)),
      BinOpKind::Ne => Ok(Value::bool(lhs != rhs, span)),
      _ => Err(error::eval::unknown_binop(span, *binop)),
    }
  }

  /// Evaluates a binary operation expression for floats.
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
      BinOpKind::Rem => Ok(Value::float(lhs % rhs, span)),
      BinOpKind::Lt => Ok(Value::bool(lhs < rhs, span)),
      BinOpKind::Gt => Ok(Value::bool(lhs > rhs, span)),
      BinOpKind::Le => Ok(Value::bool(lhs <= rhs, span)),
      BinOpKind::Ge => Ok(Value::bool(lhs >= rhs, span)),
      BinOpKind::Eq => Ok(Value::bool(lhs == rhs, span)),
      BinOpKind::Ne => Ok(Value::bool(lhs != rhs, span)),
      _ => Err(error::eval::unknown_binop(span, *binop)),
    }
  }

  /// Evaluates a binary operation expression for booleans.
  fn interpret_expr_binop_bool(
    &mut self,
    binop: &BinOp,
    lhs: &bool,
    rhs: &bool,
    span: Span,
  ) -> Result<Value> {
    match binop.kind {
      BinOpKind::And => Ok(Value::bool(*lhs && *rhs, span)),
      BinOpKind::Or => Ok(Value::bool(*lhs || *rhs, span)),
      BinOpKind::BitAnd => Ok(Value::bool(*lhs & *rhs, span)),
      BinOpKind::BitOr => Ok(Value::bool(*lhs | *rhs, span)),
      BinOpKind::BitXor => Ok(Value::bool(*lhs ^ *rhs, span)),
      _ => Err(error::eval::unknown_binop(span, *binop)),
    }
  }

  /// Evaluates an assignment expression.
  fn interpret_expr_assign(
    &mut self,
    assignee: &Expr,
    value: &Expr,
  ) -> Result<Value> {
    let value = self.interpret_expr(value)?;
    let name = *assignee.as_symbol();

    self.scope_map.add_var(name, value.clone())?;

    Ok(value)
  }

  /// Evaluates an assignment operator expression.
  fn interpret_expr_assign_op(
    &mut self,
    binop: &BinOp,
    assignee: &Expr,
    value: &Expr,
    span: Span,
  ) -> Result<Value> {
    let name = assignee.as_symbol();

    let lhs = match self.scope_map.var(name) {
      Some(value) => value.to_owned(),
      None => return Err(error::eval::not_found_var(span, *name)),
    };

    self.scope_map.add_var(*name, lhs.to_owned())?;

    let rhs = self.interpret_expr(value)?;

    match binop.kind {
      BinOpKind::Add => Ok(lhs + rhs),
      BinOpKind::Sub => Ok(lhs - rhs),
      BinOpKind::Mul => Ok(lhs * rhs),
      BinOpKind::Div => Ok(lhs / rhs),
      BinOpKind::Rem => Ok(lhs % rhs),
      // todo — should be `unknown assignop` error instead.
      _ => Err(error::eval::unknown_binop(span, *binop)),
    }
  }

  /// Evaluates an array expression.
  fn interpret_expr_array(
    &mut self,
    elmts: &[Expr],
    span: Span,
  ) -> Result<Value> {
    let mut array = Vec::with_capacity(elmts.len());

    for elmt in elmts {
      array.push(self.interpret_expr(elmt)?);
    }

    Ok(Value::array(array, span))
  }

  /// Evaluates an array access expression.
  fn interpret_expr_array_access(
    &mut self,
    indexed: &Expr,
    index: &Expr,
    span: Span,
  ) -> Result<Value> {
    let indexed = self.interpret_expr(indexed)?;
    let index = self.interpret_expr(index)?;

    if let (ValueKind::Array(array), ValueKind::Int(ref int)) =
      (&indexed.kind, &index.kind)
    {
      return self.interpret_expr_array_access_int(array, int, span);
    };

    Err(error::eval::invalid_array_access(span, indexed, index))
  }

  /// Evaluates an array access expression for integers.
  fn interpret_expr_array_access_int(
    &mut self,
    indexed: &[Value],
    index: &i64,
    span: Span,
  ) -> Result<Value> {
    match indexed.get(*index as usize) {
      Some(value) => Ok(value.to_owned()),
      None => Err(error::eval::not_found_array_elmt(span, *index)),
    }
  }
}

/// Evaluates an AST — see also [`Interpreter::interpret`].
///
/// #### examples.
///
/// ```ignore
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
